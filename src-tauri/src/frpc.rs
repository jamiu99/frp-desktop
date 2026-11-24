//! frpc 子进程管理（按 frps 服务端聚合）。
//!
//! 设计：
//! - 一个 frps 服务端 = 一个 frpc 进程（承载该 server 下所有启用的 proxy）
//! - toml 路径：app_data_dir/frpc-configs/server-{server_id}.toml
//! - 启动 / 停止某个 proxy → 该 server 的 frpc 进程重写配置 + 重启
//!   （frpc 也支持 reload-config 但需要开 admin 端口；重启简单可靠）
//! - frpc 二进制查找：用户设置 → sidecar → PATH
//! - Windows 默认隐藏控制台窗口（CREATE_NO_WINDOW），可在设置里打开

use base64::{engine::general_purpose::STANDARD, Engine};
use serde::Serialize;
use std::collections::{HashMap, VecDeque};
use std::env;
use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::thread;
use tauri::{AppHandle, Emitter, Manager};

#[cfg(windows)]
use std::os::windows::process::CommandExt;

use crate::store::{FrpsServer, Proxy, Store};

pub const MAX_LOG_LINES: usize = 500;

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

#[derive(Debug, Default, Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProxyRunStatus {
    #[default]
    Stopped,
    Starting,
    Running,
    Crashed,
}

/// 一个 frps 服务端对应的 frpc 子进程信息
struct ServerProc {
    child: Child,
    pid: u32,
    proxy_ids: Vec<String>,
    output: SharedOutput,
}

#[derive(Default)]
struct ProcessOutput {
    status: ProxyRunStatus,
    log: VecDeque<String>,
    last_error: Option<String>,
}

type SharedOutput = Arc<Mutex<ProcessOutput>>;

#[derive(Default)]
pub struct Runtime {
    /// key = server_id
    procs: Mutex<HashMap<String, ServerProc>>,
    finished: Mutex<HashMap<String, SharedOutput>>,
    operations: Mutex<()>,
}

impl Runtime {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn snapshot(&self, server_id: &str, lines: usize) -> ServerRuntime {
        let procs = self.procs.lock().unwrap();
        let finished = self.finished.lock().unwrap();
        snapshot(server_id, &procs, &finished, lines)
    }

    fn active_proxy_ids(&self, server_id: &str) -> Vec<String> {
        self.procs
            .lock()
            .unwrap()
            .get(server_id)
            .map(|p| p.proxy_ids.clone())
            .unwrap_or_default()
    }

    pub fn includes_proxy(&self, server_id: &str, proxy_id: &str) -> bool {
        self.procs
            .lock()
            .unwrap()
            .get(server_id)
            .is_some_and(|p| p.proxy_ids.iter().any(|id| id == proxy_id))
    }

    fn record_failure(&self, app: &AppHandle, server_id: &str, error: &str) {
        let mut finished = self.finished.lock().unwrap();
        let mut process = finished
            .entry(server_id.to_string())
            .or_default()
            .lock()
            .unwrap();
        process.status = ProxyRunStatus::Crashed;
        process.last_error = Some(error.to_string());
        let _ = app.emit(
            "server-status",
            StatusEvent {
                server_id: server_id.to_string(),
                status: ProxyRunStatus::Crashed,
            },
        );
    }
}

fn remember_exit(runtime: &Runtime, server_id: &str, process: ServerProc, status: ProxyRunStatus) {
    process.output.lock().unwrap().status = status;
    runtime
        .finished
        .lock()
        .unwrap()
        .insert(server_id.to_string(), process.output);
}

// ---------- helpers ----------

fn frpc_path(app: &AppHandle) -> Result<PathBuf, String> {
    if let Some(s) = app.try_state::<Store>() {
        let snap = s.snapshot();
        if let Some(p) = snap.settings.frpc_path.as_ref() {
            let p = PathBuf::from(p);
            if p.exists() {
                return Ok(p);
            }
        }
    }
    let exe_name = if cfg!(target_os = "windows") {
        "frpc.exe"
    } else {
        "frpc"
    };
    if let Ok(exe) = env::current_exe() {
        if let Some(dir) = exe.parent() {
            let candidate = dir.join(exe_name);
            if candidate.exists() {
                return Ok(candidate);
            }
        }
    }
    if let Ok(res_dir) = app.path().resource_dir() {
        let candidate = res_dir.join(exe_name);
        if candidate.exists() {
            return Ok(candidate);
        }
    }
    if which_in_path(exe_name).is_some() {
        return Ok(PathBuf::from(exe_name));
    }
    Err("找不到 frpc 二进制：未在设置里指定路径，sidecar 缺失，PATH 中也没有。".into())
}

/// 去掉 ANSI 转义序列（如 \x1b[1;34m），frpc 控制台输出带颜色会变乱码。
fn strip_ansi(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\x1b' {
            // ESC：吃掉 '[' 直到字母（CSI 序列结束符 @-~）
            if chars.peek() == Some(&'[') {
                chars.next();
                while let Some(&n) = chars.peek() {
                    chars.next();
                    if n.is_ascii_alphabetic() {
                        break;
                    }
                }
            }
        } else {
            out.push(c);
        }
    }
    out
}

fn which_in_path(name: &str) -> Option<PathBuf> {
    let path = env::var_os("PATH")?;
    for p in env::split_paths(&path) {
        let candidate = p.join(name);
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    None
}

fn config_dir(app: &AppHandle) -> Result<PathBuf, String> {
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("app_data_dir: {e}"))?
        .join("frpc-configs");
    fs::create_dir_all(&dir).map_err(|e| format!("create dir: {e}"))?;
    Ok(dir)
}

fn render_toml(server: &FrpsServer, proxies: &[&Proxy]) -> String {
    let mut out = String::new();
    out.push_str(&format!("serverAddr = {}\n", toml_string(&server.host)));
    out.push_str(&format!("serverPort = {}\n", server.port));
    if !server.token.is_empty() {
        out.push_str(&format!(
            "auth.method = \"token\"\nauth.token = {}\n",
            toml_string(&server.token)
        ));
    }
    out.push_str("log.to = \"console\"\nlog.level = \"info\"\n");

    for p in proxies {
        out.push_str("\n[[proxies]]\n");
        out.push_str(&format!("name = {}\n", toml_string(&p.name)));
        let type_str = p.proxy_type.as_str();
        out.push_str(&format!("type = \"{type_str}\"\n"));
        out.push_str(&format!("localIP = {}\n", toml_string(&p.local_ip)));
        out.push_str(&format!("localPort = {}\n", p.local_port));
        if let Some(rp) = p.remote_port {
            out.push_str(&format!("remotePort = {}\n", rp));
        }
        if !p.custom_domains.is_empty() {
            let arr = p
                .custom_domains
                .iter()
                .map(|d| toml_string(d))
                .collect::<Vec<_>>()
                .join(", ");
            out.push_str(&format!("customDomains = [{}]\n", arr));
        }
    }
    out
}

fn toml_string(value: &str) -> String {
    // frpc renders Go templates before parsing TOML. Escape template delimiters as data.
    serde_json::to_string(value)
        .unwrap()
        .replace('\u{7f}', "\\u007f")
        .replace('{', "\\u007b")
}

fn write_config(
    app: &AppHandle,
    server: &FrpsServer,
    proxies: &[&Proxy],
) -> Result<PathBuf, String> {
    let dir = config_dir(app)?;
    let path = dir.join(format!("server-{}.toml", server.id));
    let mut tmp =
        tempfile::NamedTempFile::new_in(&dir).map_err(|e| format!("create temp config: {e}"))?;
    tmp.write_all(render_toml(server, proxies).as_bytes())
        .map_err(|e| format!("write toml: {e}"))?;
    tmp.persist(&path).map_err(|e| format!("save toml: {e}"))?;
    Ok(path)
}

fn show_console(app: &AppHandle) -> bool {
    app.try_state::<Store>()
        .map(|s| s.snapshot().settings.show_frpc_console)
        .unwrap_or(false)
}

fn build_command(bin: &PathBuf, cfg: &PathBuf, _show_console: bool) -> Command {
    let mut cmd = Command::new(bin);
    cmd.arg("-c").arg(cfg);
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());
    cmd.stdin(Stdio::null());

    #[cfg(windows)]
    {
        if !_show_console {
            // 默认隐藏 frpc 控制台黑框
            cmd.creation_flags(CREATE_NO_WINDOW);
        }
    }
    cmd
}

fn spawn_log_pump<R: std::io::Read + Send + 'static>(
    reader: R,
    output: SharedOutput,
    redactor: Arc<LogRedactor>,
    app: AppHandle,
    server_id: String,
) {
    thread::spawn(move || {
        let buf = BufReader::new(reader);
        for line in buf.lines() {
            let Ok(line) = line else { break };
            let line = strip_ansi(&line);
            let lower = line.to_lowercase();
            let line = redactor.redact(line);
            let mut output = output.lock().unwrap();
            if lower.contains("login to server success") || lower.contains("start proxy success") {
                if matches!(output.status, ProxyRunStatus::Starting) {
                    output.status = ProxyRunStatus::Running;
                    let _ = app.emit(
                        "server-status",
                        StatusEvent {
                            server_id: server_id.clone(),
                            status: ProxyRunStatus::Running,
                        },
                    );
                }
            } else if lower.contains("error") || lower.contains("failed") {
                output.last_error = Some(line.clone());
            }

            if output.log.len() >= MAX_LOG_LINES {
                output.log.pop_front();
            }
            output.log.push_back(line.clone());
            drop(output);

            let _ = app.emit(
                "server-log",
                LogEvent {
                    server_id: server_id.clone(),
                    line,
                },
            );
        }
        // 流结束 → 进程退出
    });
}

struct LogRedactor(Vec<String>);

impl LogRedactor {
    fn new(server: &FrpsServer) -> Self {
        let mut patterns = Vec::new();
        for secret in [
            Some(server.token.as_str()),
            server.dashboard_pass.as_deref(),
        ]
        .into_iter()
        .flatten()
        {
            if !secret.is_empty() {
                let escaped = toml_string(secret);
                patterns.push(escaped[1..escaped.len() - 1].to_owned());
                patterns.push(secret.to_owned());
            }
        }
        if let (Some(user), Some(pass)) = (&server.dashboard_user, &server.dashboard_pass) {
            patterns.push(STANDARD.encode(format!("{user}:{pass}")));
        }
        patterns.sort_unstable_by(|a, b| b.len().cmp(&a.len()).then_with(|| a.cmp(b)));
        patterns.dedup();
        Self(patterns)
    }

    fn redact(&self, mut line: String) -> String {
        for pattern in &self.0 {
            line = line.replace(pattern, "[已隐藏]");
        }
        const MAX_LINE_BYTES: usize = 4096;
        if line.len() > MAX_LINE_BYTES {
            let end = line.floor_char_boundary(MAX_LINE_BYTES);
            line.truncate(end);
            line.push('…');
        }
        line
    }
}

#[derive(Serialize, Clone)]
struct LogEvent {
    server_id: String,
    line: String,
}

#[derive(Serialize, Clone)]
struct StatusEvent {
    server_id: String,
    status: ProxyRunStatus,
}

// 真正的核心：(re)launch 一个 server 的 frpc 进程
// 调用方持有 store snapshot，传入要跑哪些 proxy。proxies 为空时 = 停止
fn relaunch_server(
    app: &AppHandle,
    runtime: &Runtime,
    server: &FrpsServer,
    enabled_proxies: &[&Proxy],
) -> Result<Option<u32>, String> {
    if enabled_proxies.is_empty() {
        kill_server(app, runtime, &server.id);
        return Ok(None);
    }

    // Prepare the replacement before interrupting existing proxies.
    let bin = frpc_path(app)?;
    let cfg = write_config(app, server, enabled_proxies)?;
    let mut cmd = build_command(&bin, &cfg, show_console(app));
    stop_process(runtime, &server.id);

    let mut child = cmd.spawn().map_err(|e| format!("启动 frpc 失败: {e}"))?;
    let pid = child.id();

    let output = Arc::new(Mutex::new(ProcessOutput {
        status: ProxyRunStatus::Starting,
        ..Default::default()
    }));
    let redactor = Arc::new(LogRedactor::new(server));
    let stdout = child.stdout.take();
    let stderr = child.stderr.take();
    runtime.procs.lock().unwrap().insert(
        server.id.clone(),
        ServerProc {
            child,
            pid,
            proxy_ids: enabled_proxies.iter().map(|p| p.id.clone()).collect(),
            output: output.clone(),
        },
    );
    let _ = app.emit(
        "server-status",
        StatusEvent {
            server_id: server.id.clone(),
            status: ProxyRunStatus::Starting,
        },
    );

    if let Some(out) = stdout {
        spawn_log_pump(
            out,
            output.clone(),
            redactor.clone(),
            app.clone(),
            server.id.clone(),
        );
    }
    if let Some(err) = stderr {
        spawn_log_pump(
            err,
            output.clone(),
            redactor,
            app.clone(),
            server.id.clone(),
        );
    }

    let app_for_wait = app.clone();
    let server_id = server.id.clone();
    thread::spawn(move || poll_until_exit(&app_for_wait, server_id, output));

    Ok(Some(pid))
}

fn poll_until_exit(app: &AppHandle, server_id: String, output: SharedOutput) {
    loop {
        std::thread::sleep(std::time::Duration::from_millis(500));
        let Some(rt) = app.try_state::<Runtime>() else {
            return;
        };
        let mut g = rt.procs.lock().unwrap();
        let Some(p) = g.get_mut(&server_id) else {
            return;
        };
        if !Arc::ptr_eq(&p.output, &output) {
            return;
        }
        match p.child.try_wait() {
            Ok(Some(exit)) => {
                p.output
                    .lock()
                    .unwrap()
                    .last_error
                    .get_or_insert_with(|| format!("frpc 已退出 ({exit})"));
                let process = g.remove(&server_id).unwrap();
                remember_exit(&rt, &server_id, process, ProxyRunStatus::Crashed);
                let _ = app.emit(
                    "server-status",
                    StatusEvent {
                        server_id,
                        status: ProxyRunStatus::Crashed,
                    },
                );
                return;
            }
            Ok(None) => {}
            Err(_) => return,
        }
    }
}

fn enabled_proxies_for_server<'a>(proxies: &'a [Proxy], server_id: &str) -> Vec<&'a Proxy> {
    proxies
        .iter()
        .filter(|p| p.server_id == server_id && p.enabled)
        .collect()
}

// ---------- commands ----------

#[derive(Serialize, Clone, Copy)]
#[serde(rename_all = "snake_case")]
enum ProcessChange {
    None,
    Start,
    Restart,
    Stop,
}

#[derive(Serialize)]
pub struct ProxyChange {
    proxy_id: String,
    server_id: String,
    previous_enabled: bool,
    enabled: bool,
    process_change: ProcessChange,
    affected_proxies: Vec<String>,
    dry_run: bool,
}

pub fn change_proxy(
    app: &AppHandle,
    store: &Store,
    runtime: &Runtime,
    proxy_id: &str,
    enabled: bool,
    dry_run: bool,
) -> Result<ProxyChange, String> {
    let _operation = runtime.operations.lock().unwrap();
    let mut data = store.snapshot();
    let proxy = data
        .proxies
        .iter_mut()
        .find(|p| p.id == proxy_id)
        .ok_or("proxy 不存在")?;
    let server = data
        .servers
        .iter()
        .find(|s| s.id == proxy.server_id)
        .ok_or("服务端不存在")?;
    let previous_enabled = proxy.enabled;
    proxy.enabled = enabled;
    let desired = enabled_proxies_for_server(&data.proxies, &server.id);
    let active_ids = runtime.active_proxy_ids(&server.id);
    let is_active = active_ids.iter().any(|id| id == proxy_id);
    let process_change = match (enabled, is_active) {
        (true, false) if active_ids.is_empty() => ProcessChange::Start,
        (true, false) => ProcessChange::Restart,
        (false, true) if desired.is_empty() => ProcessChange::Stop,
        (false, true) => ProcessChange::Restart,
        _ => ProcessChange::None,
    };
    let affected_proxies = data
        .proxies
        .iter()
        .filter(|p| {
            p.id != proxy_id
                && active_ids.contains(&p.id)
                && matches!(process_change, ProcessChange::Restart | ProcessChange::Stop)
        })
        .map(|p| p.name.clone())
        .collect();
    let plan = ProxyChange {
        proxy_id: proxy_id.to_string(),
        server_id: server.id.clone(),
        previous_enabled,
        enabled,
        process_change,
        affected_proxies,
        dry_run,
    };
    if dry_run {
        return Ok(plan);
    }

    let persist_enabled = |value| {
        store.mutate(|data| {
            let p = data
                .proxies
                .iter_mut()
                .find(|p| p.id == proxy_id)
                .ok_or("proxy 不存在")?;
            p.enabled = value;
            Ok(())
        })
    };
    if previous_enabled != enabled {
        persist_enabled(enabled)?;
    }
    if matches!(process_change, ProcessChange::None) {
        return Ok(plan);
    }
    if let Err(error) = relaunch_server(app, runtime, server, &desired) {
        let mut error = error;
        if previous_enabled != enabled {
            if let Err(rollback) = persist_enabled(previous_enabled) {
                error.push_str(&format!("；恢复 proxy 配置失败：{rollback}"));
            }
        }
        if runtime.active_proxy_ids(&server.id).is_empty() {
            runtime.record_failure(app, &server.id, &error);
            if !active_ids.is_empty() {
                let previous: Vec<_> = data
                    .proxies
                    .iter()
                    .filter(|p| active_ids.contains(&p.id))
                    .collect();
                if let Err(restart) = relaunch_server(app, runtime, server, &previous) {
                    error.push_str(&format!("；恢复原 frpc 进程失败：{restart}"));
                    runtime.record_failure(app, &server.id, &error);
                }
            }
        }
        return Err(error);
    }
    Ok(plan)
}

#[tauri::command]
pub fn start_proxy(
    app: AppHandle,
    store: tauri::State<Store>,
    runtime: tauri::State<Runtime>,
    proxy_id: String,
) -> Result<(), String> {
    change_proxy(&app, &store, &runtime, &proxy_id, true, false)?;
    Ok(())
}

#[tauri::command]
pub fn stop_proxy(
    app: AppHandle,
    store: tauri::State<Store>,
    runtime: tauri::State<Runtime>,
    proxy_id: String,
) -> Result<(), String> {
    change_proxy(&app, &store, &runtime, &proxy_id, false, false)?;
    Ok(())
}

/// 启动整个 server 的所有启用 proxy（一次起一组）
#[tauri::command]
pub fn start_server(
    app: AppHandle,
    store: tauri::State<Store>,
    runtime: tauri::State<Runtime>,
    server_id: String,
) -> Result<(), String> {
    let _operation = runtime.operations.lock().unwrap();
    let snap = store.snapshot();
    let server = snap
        .servers
        .iter()
        .find(|s| s.id == server_id)
        .ok_or_else(|| "服务端不存在".to_string())?;
    let enabled = enabled_proxies_for_server(&snap.proxies, &server.id);
    if enabled.is_empty() {
        return Err("该服务端下没有启用的 proxy".into());
    }
    if let Err(error) = relaunch_server(&app, &runtime, server, &enabled) {
        if runtime.active_proxy_ids(&server_id).is_empty() {
            runtime.record_failure(&app, &server_id, &error);
        }
        return Err(error);
    }
    Ok(())
}

/// 停掉整个 server
#[tauri::command]
pub fn stop_server(
    app: AppHandle,
    runtime: tauri::State<Runtime>,
    server_id: String,
) -> Result<(), String> {
    let _operation = runtime.operations.lock().unwrap();
    kill_server(&app, &runtime, &server_id);
    Ok(())
}

fn stop_process(runtime: &Runtime, server_id: &str) {
    let mut g = runtime.procs.lock().unwrap();
    if let Some(mut p) = g.remove(server_id) {
        p.output.lock().unwrap().status = ProxyRunStatus::Stopped;
        let _ = p.child.kill();
        let _ = p.child.wait();
        remember_exit(runtime, server_id, p, ProxyRunStatus::Stopped);
    }
}

fn kill_server(app: &AppHandle, runtime: &Runtime, server_id: &str) {
    stop_process(runtime, server_id);
    if let Some(process) = runtime.finished.lock().unwrap().get_mut(server_id) {
        process.lock().unwrap().status = ProxyRunStatus::Stopped;
    }
    let _ = app.emit(
        "server-status",
        StatusEvent {
            server_id: server_id.to_string(),
            status: ProxyRunStatus::Stopped,
        },
    );
}

/// 导入配置后调用：只处理当前正在跑的 frpc。
/// - 服务端已不存在（覆盖导入 / 被删）→ 停掉
/// - 服务端配置被导入更新过（touched）→ 按新配置重启
/// - 其他保持不动，不打断无关隧道
pub fn reconcile_after_import(
    app: &AppHandle,
    runtime: &Runtime,
    store: &Store,
    touched: &std::collections::HashSet<String>,
) {
    let _operation = runtime.operations.lock().unwrap();
    let running: Vec<String> = runtime.procs.lock().unwrap().keys().cloned().collect();
    let snap = store.snapshot();
    for id in running {
        match snap.servers.iter().find(|s| s.id == id) {
            None => kill_server(app, runtime, &id),
            Some(server) if touched.contains(&id) => {
                let enabled = enabled_proxies_for_server(&snap.proxies, &id);
                if let Err(e) = relaunch_server(app, runtime, server, &enabled) {
                    if runtime.active_proxy_ids(&id).is_empty() {
                        runtime.record_failure(app, &id, &e);
                    }
                    eprintln!("[frpc] relaunch {id} after import failed: {e}");
                }
            }
            Some(_) => {}
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct ServerRuntime {
    pub server_id: String,
    pub status: ProxyRunStatus,
    pub pid: Option<u32>,
    pub last_error: Option<String>,
    pub log_tail: Vec<String>,
}

fn snapshot(
    server_id: &str,
    procs: &HashMap<String, ServerProc>,
    finished: &HashMap<String, SharedOutput>,
    lines: usize,
) -> ServerRuntime {
    let mut result = ServerRuntime {
        server_id: server_id.to_owned(),
        status: ProxyRunStatus::Stopped,
        pid: None,
        last_error: None,
        log_tail: Vec::new(),
    };
    let output = if let Some(process) = procs.get(server_id) {
        result.pid = Some(process.pid);
        &process.output
    } else if let Some(process) = finished.get(server_id) {
        process
    } else {
        return result;
    };
    let output = output.lock().unwrap();
    result.status = output.status;
    result.log_tail = output
        .log
        .iter()
        .skip(output.log.len().saturating_sub(lines))
        .cloned()
        .collect();
    result.last_error = output.last_error.clone();
    result
}

#[tauri::command]
pub fn list_runtime(runtime: tauri::State<Runtime>) -> Vec<ServerRuntime> {
    let procs = runtime.procs.lock().unwrap();
    let finished = runtime.finished.lock().unwrap();
    procs
        .keys()
        .chain(finished.keys().filter(|id| !procs.contains_key(*id)))
        .map(|id| snapshot(id, &procs, &finished, MAX_LOG_LINES))
        .collect()
}

#[tauri::command]
pub fn server_logs(runtime: tauri::State<Runtime>, server_id: String) -> Vec<String> {
    runtime.snapshot(&server_id, MAX_LOG_LINES).log_tail
}

#[tauri::command]
pub fn check_frpc(app: AppHandle) -> Result<String, String> {
    let bin = frpc_path(&app)?;
    let mut cmd = Command::new(&bin);
    cmd.arg("--version");
    #[cfg(windows)]
    cmd.creation_flags(CREATE_NO_WINDOW);
    let out = cmd.output().map_err(|e| format!("调用 frpc 失败: {e}"))?;
    let v = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if v.is_empty() {
        Ok(format!("frpc 在 {} (版本读取失败)", bin.display()))
    } else {
        Ok(format!("{v} ({})", bin.display()))
    }
}

pub fn shutdown_all(runtime: &Runtime) {
    let _operation = runtime.operations.lock().unwrap();
    let mut g = runtime.procs.lock().unwrap();
    for (id, mut p) in g.drain() {
        p.output.lock().unwrap().status = ProxyRunStatus::Stopped;
        let _ = p.child.kill();
        // 必须 wait，否则 Windows 上 kill 是异步的，进程还没退出就去覆盖
        // frpc.exe 会失败（文件被占用），且残留孤儿进程
        let _ = p.child.wait();
        remember_exit(runtime, &id, p, ProxyRunStatus::Stopped);
    }
}

/// 杀掉上次异常退出（崩溃 / 任务管理器强杀）遗留的 frpc 孤儿进程。
/// 只认命令行里带本应用 frpc-configs 目录的进程，不误伤用户自己跑的 frpc。
fn kill_orphan_frpc(app: &AppHandle, runtime: &Runtime) {
    use sysinfo::{ProcessRefreshKind, RefreshKind, System, UpdateKind};

    let Ok(dir) = config_dir(app) else { return };
    let managed_pids: Vec<_> = runtime
        .procs
        .lock()
        .unwrap()
        .values()
        .map(|p| p.pid)
        .collect();
    let marker = dir.to_string_lossy().to_lowercase();
    let sys = System::new_with_specifics(
        RefreshKind::new().with_processes(ProcessRefreshKind::new().with_cmd(UpdateKind::Always)),
    );
    for proc_ in sys.processes().values() {
        let ours = proc_.cmd().iter().any(|a| {
            std::path::Path::new(a)
                .to_string_lossy()
                .to_lowercase()
                .contains(&marker)
        });
        // kill 成功才 wait：等它真正退出、释放 remotePort，再重新拉起
        if ours && !managed_pids.contains(&proc_.pid().as_u32()) && proc_.kill() {
            proc_.wait();
        }
    }
}

/// 应用启动时调用：按 store 里的 enabled 状态把 frpc 拉起来。
/// 之前只持久化了 enabled，却不在启动时起进程，导致界面显示「已启用」但实际没在跑。
/// 放后台线程，不阻塞窗口创建；前端先 listen 再 refresh，不会丢状态。
pub fn restore_on_startup(app: AppHandle) {
    thread::spawn(move || {
        let (Some(store), Some(runtime)) = (app.try_state::<Store>(), app.try_state::<Runtime>())
        else {
            return;
        };
        let _operation = runtime.operations.lock().unwrap();
        kill_orphan_frpc(&app, &runtime);
        let snap = store.snapshot();
        for server in &snap.servers {
            let enabled = enabled_proxies_for_server(&snap.proxies, &server.id);
            if enabled.is_empty() || !runtime.active_proxy_ids(&server.id).is_empty() {
                continue;
            }
            if let Err(e) = relaunch_server(&app, &runtime, server, &enabled) {
                runtime.record_failure(&app, &server.id, &e);
                eprintln!("[frpc] restore server {} failed: {e}", server.id);
            }
        }
    });
}

/// 应用更新/退出前调用：停掉所有 frpc 子进程并等待其真正退出。
/// 解决 Windows 自动更新时 frpc.exe 被占用无法覆盖、以及子进程变孤儿的问题。
#[tauri::command]
pub fn stop_all_proxies(runtime: tauri::State<Runtime>) -> Result<(), String> {
    shutdown_all(&runtime);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frpc_input_remains_a_toml_string_and_bypasses_go_template_rendering() {
        let raw = "a\"\nlocalPort = 9999\n{{ .Envs.FRP_DESKTOP_FIXTURE }}\u{7f}";
        let encoded = toml_string(raw);
        assert!(!encoded.contains("{{"));
        let parsed: toml::Table = format!("value = {encoded}").parse().unwrap();
        assert_eq!(parsed["value"].as_str(), Some(raw));
    }

    #[test]
    fn recent_exit_retains_redacted_logs_without_an_active_pid() {
        let server = FrpsServer {
            id: "fixture".into(),
            name: "fixture".into(),
            host: "example.invalid".into(),
            port: 7000,
            token: "fixture-{token}".into(),
            dashboard_url: None,
            dashboard_user: Some("fixture".into()),
            dashboard_pass: Some("fixture-pass".into()),
            created_at: chrono::Utc::now(),
        };
        let redactor = LogRedactor::new(&server);
        let line = redactor.redact(format!(
            "error {} {} {} {}",
            server.token,
            toml_string(&server.token),
            server.dashboard_pass.as_deref().unwrap(),
            STANDARD.encode("fixture:fixture-pass")
        ));
        let runtime = Runtime::new();
        runtime.finished.lock().unwrap().insert(
            server.id.clone(),
            Arc::new(Mutex::new(ProcessOutput {
                status: ProxyRunStatus::Crashed,
                last_error: Some(line.clone()),
                log: VecDeque::from(["older".into(), line]),
            })),
        );
        let snapshot = runtime.snapshot(&server.id, 1);
        assert!(matches!(snapshot.status, ProxyRunStatus::Crashed));
        assert!(snapshot.pid.is_none());
        assert_eq!(
            snapshot.log_tail,
            ["error [已隐藏] \"[已隐藏]\" [已隐藏] [已隐藏]"]
        );
        assert_eq!(snapshot.last_error.as_ref(), snapshot.log_tail.first());
    }
}
