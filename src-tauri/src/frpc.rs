//! frpc 子进程管理（按 frps 服务端聚合）。
//!
//! 设计：
//! - 一个 frps 服务端 = 一个 frpc 进程（承载该 server 下所有启用的 proxy）
//! - toml 路径：app_data_dir/frpc-configs/server-{server_id}.toml
//! - 启动 / 停止某个 proxy → 该 server 的 frpc 进程重写配置 + 重启
//!   （frpc 也支持 reload-config 但需要开 admin 端口；重启简单可靠）
//! - frpc 二进制查找：用户设置 → sidecar → PATH
//! - Windows 默认隐藏控制台窗口（CREATE_NO_WINDOW），可在设置里打开

use serde::Serialize;
use std::collections::HashMap;
use std::env;
use std::fs;
use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::thread;
use tauri::{AppHandle, Emitter, Manager};

#[cfg(windows)]
use std::os::windows::process::CommandExt;

use crate::store::{FrpsServer, Proxy, ProxyType, Store};

const MAX_LOG_LINES: usize = 500;

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProxyRunStatus {
    Stopped,
    Starting,
    Running,
    Crashed,
}

/// 一个 frps 服务端对应的 frpc 子进程信息
struct ServerProc {
    child: Child,
    pid: u32,
    log: Arc<Mutex<Vec<String>>>,
    status: Arc<Mutex<ProxyRunStatus>>,
    last_error: Arc<Mutex<Option<String>>>,
}

#[derive(Default)]
pub struct Runtime {
    /// key = server_id
    procs: Mutex<HashMap<String, ServerProc>>,
}

impl Runtime {
    pub fn new() -> Self {
        Self::default()
    }
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
    let exe_name = if cfg!(target_os = "windows") { "frpc.exe" } else { "frpc" };
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
    out.push_str(&format!("serverAddr = \"{}\"\n", server.host));
    out.push_str(&format!("serverPort = {}\n", server.port));
    if !server.token.is_empty() {
        out.push_str(&format!(
            "auth.method = \"token\"\nauth.token = \"{}\"\n",
            server.token
        ));
    }
    out.push_str("log.to = \"console\"\nlog.level = \"info\"\n");

    for p in proxies {
        out.push_str("\n[[proxies]]\n");
        out.push_str(&format!("name = \"{}\"\n", p.name));
        let type_str = match p.proxy_type {
            ProxyType::Tcp => "tcp",
            ProxyType::Udp => "udp",
            ProxyType::Http => "http",
            ProxyType::Https => "https",
            ProxyType::Stcp => "stcp",
        };
        out.push_str(&format!("type = \"{type_str}\"\n"));
        out.push_str(&format!("localIP = \"{}\"\n", p.local_ip));
        out.push_str(&format!("localPort = {}\n", p.local_port));
        if let Some(rp) = p.remote_port {
            out.push_str(&format!("remotePort = {}\n", rp));
        }
        if !p.custom_domains.is_empty() {
            let arr = p
                .custom_domains
                .iter()
                .map(|d| format!("\"{}\"", d))
                .collect::<Vec<_>>()
                .join(", ");
            out.push_str(&format!("customDomains = [{}]\n", arr));
        }
    }
    out
}

fn write_config(
    app: &AppHandle,
    server: &FrpsServer,
    proxies: &[&Proxy],
) -> Result<PathBuf, String> {
    let dir = config_dir(app)?;
    let path = dir.join(format!("server-{}.toml", server.id));
    fs::write(&path, render_toml(server, proxies)).map_err(|e| format!("write toml: {e}"))?;
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
    log: Arc<Mutex<Vec<String>>>,
    status: Arc<Mutex<ProxyRunStatus>>,
    last_error: Arc<Mutex<Option<String>>>,
    app: AppHandle,
    server_id: String,
) {
    thread::spawn(move || {
        let buf = BufReader::new(reader);
        for line in buf.lines() {
            let Ok(line) = line else { break };
            let line = strip_ansi(&line); // frpc 控制台输出带颜色转义码，去掉
            let lower = line.to_lowercase();
            if lower.contains("login to server success")
                || lower.contains("start proxy success")
            {
                *status.lock().unwrap() = ProxyRunStatus::Running;
                let _ = app.emit(
                    "server-status",
                    StatusEvent {
                        server_id: server_id.clone(),
                        status: ProxyRunStatus::Running,
                    },
                );
            } else if lower.contains("error") || lower.contains("failed") {
                *last_error.lock().unwrap() = Some(line.clone());
            }

            {
                let mut g = log.lock().unwrap();
                if g.len() >= MAX_LOG_LINES {
                    g.remove(0);
                }
                g.push(line.clone());
            }

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
    // 先停掉旧的（如果有）
    {
        let mut g = runtime.procs.lock().unwrap();
        if let Some(mut p) = g.remove(&server.id) {
            *p.status.lock().unwrap() = ProxyRunStatus::Stopped;
            let _ = p.child.kill();
            let _ = p.child.wait();
        }
    }

    if enabled_proxies.is_empty() {
        let _ = app.emit(
            "server-status",
            StatusEvent {
                server_id: server.id.clone(),
                status: ProxyRunStatus::Stopped,
            },
        );
        return Ok(None);
    }

    let bin = frpc_path(app)?;
    let cfg = write_config(app, server, enabled_proxies)?;
    let mut cmd = build_command(&bin, &cfg, show_console(app));

    let mut child = cmd.spawn().map_err(|e| format!("启动 frpc 失败: {e}"))?;
    let pid = child.id();

    let log = Arc::new(Mutex::new(Vec::<String>::new()));
    let status = Arc::new(Mutex::new(ProxyRunStatus::Starting));
    let last_error = Arc::new(Mutex::new(None::<String>));

    if let Some(out) = child.stdout.take() {
        spawn_log_pump(
            out,
            log.clone(),
            status.clone(),
            last_error.clone(),
            app.clone(),
            server.id.clone(),
        );
    }
    if let Some(err) = child.stderr.take() {
        spawn_log_pump(
            err,
            log.clone(),
            status.clone(),
            last_error.clone(),
            app.clone(),
            server.id.clone(),
        );
    }

    {
        let mut g = runtime.procs.lock().unwrap();
        g.insert(
            server.id.clone(),
            ServerProc {
                child,
                pid,
                log,
                status,
                last_error,
            },
        );
    }

    // wait 线程：进程退出时清理 procs 表 + 通知前端
    {
        let app_for_wait = app.clone();
        let server_id = server.id.clone();
        // 把 runtime 的 procs Mutex 引用通过 AppHandle 拿（Runtime 是 manage 进去的）
        thread::spawn(move || {
            poll_until_exit(&app_for_wait, server_id);
        });
    }

    let _ = app.emit(
        "server-status",
        StatusEvent {
            server_id: server.id.clone(),
            status: ProxyRunStatus::Starting,
        },
    );

    Ok(Some(pid))
}

fn poll_until_exit(app: &AppHandle, server_id: String) {
    loop {
        std::thread::sleep(std::time::Duration::from_millis(500));
        let Some(rt) = app.try_state::<Runtime>() else {
            return;
        };
        let mut g = rt.procs.lock().unwrap();
        let Some(p) = g.get_mut(&server_id) else {
            return;
        };
        match p.child.try_wait() {
            Ok(Some(_)) => {
                let was_stopped =
                    matches!(*p.status.lock().unwrap(), ProxyRunStatus::Stopped);
                let final_status = if was_stopped {
                    ProxyRunStatus::Stopped
                } else {
                    ProxyRunStatus::Crashed
                };
                *p.status.lock().unwrap() = final_status.clone();
                g.remove(&server_id);
                drop(g);
                let _ = app.emit(
                    "server-status",
                    StatusEvent {
                        server_id,
                        status: final_status,
                    },
                );
                return;
            }
            Ok(None) => {}
            Err(_) => return,
        }
    }
}

fn enabled_proxies_for_server<'a>(
    proxies: &'a [Proxy],
    server_id: &str,
) -> Vec<&'a Proxy> {
    proxies
        .iter()
        .filter(|p| p.server_id == server_id && p.enabled)
        .collect()
}

// ---------- commands ----------

/// 启动一个 proxy（实际上是把它加进 server 的 enabled 列表，重启该 server frpc）
#[tauri::command]
pub fn start_proxy(
    app: AppHandle,
    store: tauri::State<Store>,
    runtime: tauri::State<Runtime>,
    proxy_id: String,
) -> Result<(), String> {
    // 先把 enabled 标记设为 true
    store.mutate(|d| {
        let p = d
            .proxies
            .iter_mut()
            .find(|p| p.id == proxy_id)
            .ok_or_else(|| "proxy 不存在".to_string())?;
        p.enabled = true;
        Ok(())
    })?;

    let snap = store.snapshot();
    let proxy = snap
        .proxies
        .iter()
        .find(|p| p.id == proxy_id)
        .ok_or_else(|| "proxy 不存在".to_string())?;
    let server = snap
        .servers
        .iter()
        .find(|s| s.id == proxy.server_id)
        .ok_or_else(|| "服务端不存在".to_string())?;
    let enabled = enabled_proxies_for_server(&snap.proxies, &server.id);
    relaunch_server(&app, &runtime, server, &enabled)?;
    Ok(())
}

/// 停止一个 proxy（如该 server 还有其他启用 proxy → 重启 frpc 重写配置；否则杀进程）
#[tauri::command]
pub fn stop_proxy(
    app: AppHandle,
    store: tauri::State<Store>,
    runtime: tauri::State<Runtime>,
    proxy_id: String,
) -> Result<(), String> {
    store.mutate(|d| {
        let p = d
            .proxies
            .iter_mut()
            .find(|p| p.id == proxy_id)
            .ok_or_else(|| "proxy 不存在".to_string())?;
        p.enabled = false;
        Ok(())
    })?;

    let snap = store.snapshot();
    let proxy = snap
        .proxies
        .iter()
        .find(|p| p.id == proxy_id)
        .ok_or_else(|| "proxy 不存在".to_string())?;
    let Some(server) = snap.servers.iter().find(|s| s.id == proxy.server_id) else {
        return Ok(());
    };
    let enabled = enabled_proxies_for_server(&snap.proxies, &server.id);
    relaunch_server(&app, &runtime, server, &enabled)?;
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
    relaunch_server(&app, &runtime, server, &enabled)?;
    Ok(())
}

/// 停掉整个 server
#[tauri::command]
pub fn stop_server(
    app: AppHandle,
    runtime: tauri::State<Runtime>,
    server_id: String,
) -> Result<(), String> {
    let mut g = runtime.procs.lock().unwrap();
    if let Some(mut p) = g.remove(&server_id) {
        *p.status.lock().unwrap() = ProxyRunStatus::Stopped;
        let _ = p.child.kill();
        let _ = p.child.wait();
    }
    drop(g);
    let _ = app.emit(
        "server-status",
        StatusEvent {
            server_id,
            status: ProxyRunStatus::Stopped,
        },
    );
    Ok(())
}

#[derive(Debug, Clone, Serialize)]
pub struct ServerRuntime {
    pub server_id: String,
    pub status: ProxyRunStatus,
    pub pid: Option<u32>,
    pub last_error: Option<String>,
    pub log_tail: Vec<String>,
}

#[tauri::command]
pub fn list_runtime(runtime: tauri::State<Runtime>) -> Vec<ServerRuntime> {
    let g = runtime.procs.lock().unwrap();
    g.iter()
        .map(|(id, p)| ServerRuntime {
            server_id: id.clone(),
            status: p.status.lock().unwrap().clone(),
            pid: Some(p.pid),
            last_error: p.last_error.lock().unwrap().clone(),
            log_tail: p.log.lock().unwrap().clone(),
        })
        .collect()
}

#[tauri::command]
pub fn server_logs(
    runtime: tauri::State<Runtime>,
    server_id: String,
) -> Vec<String> {
    let g = runtime.procs.lock().unwrap();
    g.get(&server_id)
        .map(|p| p.log.lock().unwrap().clone())
        .unwrap_or_default()
}

#[tauri::command]
pub fn check_frpc(app: AppHandle) -> Result<String, String> {
    let bin = frpc_path(&app)?;
    let mut cmd = Command::new(&bin);
    cmd.arg("--version");
    #[cfg(windows)]
    cmd.creation_flags(CREATE_NO_WINDOW);
    let out = cmd
        .output()
        .map_err(|e| format!("调用 frpc 失败: {e}"))?;
    let v = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if v.is_empty() {
        Ok(format!("frpc 在 {} (版本读取失败)", bin.display()))
    } else {
        Ok(format!("{v} ({})", bin.display()))
    }
}

pub fn shutdown_all(runtime: &Runtime) {
    let mut g = runtime.procs.lock().unwrap();
    for (_, mut p) in g.drain() {
        *p.status.lock().unwrap() = ProxyRunStatus::Stopped;
        let _ = p.child.kill();
        // 必须 wait，否则 Windows 上 kill 是异步的，进程还没退出就去覆盖
        // frpc.exe 会失败（文件被占用），且残留孤儿进程
        let _ = p.child.wait();
    }
}

/// 应用更新/退出前调用：停掉所有 frpc 子进程并等待其真正退出。
/// 解决 Windows 自动更新时 frpc.exe 被占用无法覆盖、以及子进程变孤儿的问题。
#[tauri::command]
pub fn stop_all_proxies(runtime: tauri::State<Runtime>) -> Result<(), String> {
    shutdown_all(&runtime);
    Ok(())
}
