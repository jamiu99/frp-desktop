//! frpc 子进程管理。
//!
//! 设计：
//! - 每个启用的 proxy 有一个独立的 frpc 子进程，独立 toml 配置文件
//! - toml 文件放在 app_data_dir/frpc-configs/{proxy_id}.toml
//! - 子进程句柄、stdout 行缓冲存在 RUNNING 全局表里
//! - frpc 二进制查找顺序：
//!     1. settings.frpc_path（用户在设置里指定）
//!     2. sidecar：与 frp_desktop 二进制同目录的 frpc / frpc.exe（打包时塞）
//!     3. PATH 中的 frpc

use serde::Serialize;
use std::collections::HashMap;
use std::env;
use std::fs;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::thread;
use tauri::{AppHandle, Emitter, Manager};

use crate::store::{Proxy, ProxyType, Store};

const MAX_LOG_LINES: usize = 500;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProxyRunStatus {
    Stopped,
    Starting,
    Running,
    Crashed,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProxyRuntime {
    pub proxy_id: String,
    pub status: ProxyRunStatus,
    pub pid: Option<u32>,
    pub last_error: Option<String>,
    pub log_tail: Vec<String>,
}

struct RunningProc {
    child: Child,
    log: Arc<Mutex<Vec<String>>>,
    status: Arc<Mutex<ProxyRunStatus>>,
    last_error: Arc<Mutex<Option<String>>>,
}

#[derive(Default)]
pub struct Runtime {
    procs: Mutex<HashMap<String, RunningProc>>,
}

impl Runtime {
    pub fn new() -> Self {
        Self::default()
    }
}

// ---------- helpers ----------

fn frpc_path(app: &AppHandle) -> Result<PathBuf, String> {
    // 1. 用户在设置里显式指定的路径
    if let Some(s) = app.try_state::<Store>() {
        let snap = s.snapshot();
        if let Some(p) = snap.settings.frpc_path.as_ref() {
            let p = PathBuf::from(p);
            if p.exists() {
                return Ok(p);
            }
        }
    }

    // 2. 打包后的 sidecar（Tauri 会把 binaries/frpc-{triple}{.exe} 放在 resource 目录里
    //    或与可执行文件同目录）。dev 模式下 cargo 会把 sidecar 拷到 target/debug/。
    let exe_name = if cfg!(target_os = "windows") { "frpc.exe" } else { "frpc" };
    if let Ok(exe) = env::current_exe() {
        if let Some(dir) = exe.parent() {
            let candidate = dir.join(exe_name);
            if candidate.exists() {
                return Ok(candidate);
            }
        }
    }
    // 部分 Linux 发行版（AppImage / 包安装）会把 sidecar 放在 resource_dir
    if let Ok(res_dir) = app.path().resource_dir() {
        let candidate = res_dir.join(exe_name);
        if candidate.exists() {
            return Ok(candidate);
        }
    }

    // 3. 系统 PATH
    if which_in_path(exe_name).is_some() {
        return Ok(PathBuf::from(exe_name));
    }

    Err("找不到 frpc 二进制：未在设置里指定路径，sidecar 缺失，PATH 中也没有。请到「设置」里填 frpc 路径。".into())
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

fn render_toml(proxy: &Proxy, server: &crate::store::FrpsServer) -> String {
    let mut out = String::new();
    out.push_str(&format!("serverAddr = \"{}\"\n", server.host));
    out.push_str(&format!("serverPort = {}\n", server.port));
    if !server.token.is_empty() {
        out.push_str(&format!("auth.method = \"token\"\nauth.token = \"{}\"\n", server.token));
    }
    out.push_str("\n[[proxies]]\n");
    out.push_str(&format!("name = \"{}\"\n", proxy.name));
    let type_str = match proxy.proxy_type {
        ProxyType::Tcp => "tcp",
        ProxyType::Udp => "udp",
        ProxyType::Http => "http",
        ProxyType::Https => "https",
        ProxyType::Stcp => "stcp",
    };
    out.push_str(&format!("type = \"{type_str}\"\n"));
    out.push_str(&format!("localIP = \"{}\"\n", proxy.local_ip));
    out.push_str(&format!("localPort = {}\n", proxy.local_port));
    if let Some(rp) = proxy.remote_port {
        out.push_str(&format!("remotePort = {}\n", rp));
    }
    if !proxy.custom_domains.is_empty() {
        let arr = proxy
            .custom_domains
            .iter()
            .map(|d| format!("\"{}\"", d))
            .collect::<Vec<_>>()
            .join(", ");
        out.push_str(&format!("customDomains = [{}]\n", arr));
    }
    out
}

fn write_config(app: &AppHandle, proxy: &Proxy, server: &crate::store::FrpsServer) -> Result<PathBuf, String> {
    let dir = config_dir(app)?;
    let path = dir.join(format!("{}.toml", proxy.id));
    fs::write(&path, render_toml(proxy, server)).map_err(|e| format!("write toml: {e}"))?;
    Ok(path)
}

fn spawn_log_pump<R: std::io::Read + Send + 'static>(
    reader: R,
    log: Arc<Mutex<Vec<String>>>,
    status: Arc<Mutex<ProxyRunStatus>>,
    last_error: Arc<Mutex<Option<String>>>,
    app: AppHandle,
    proxy_id: String,
) {
    thread::spawn(move || {
        let buf = BufReader::new(reader);
        for line in buf.lines() {
            let Ok(line) = line else { break };
            // 状态推断
            let lower = line.to_lowercase();
            if lower.contains("login to server success") || lower.contains("start proxy success") {
                *status.lock().unwrap() = ProxyRunStatus::Running;
            } else if lower.contains("error") {
                *last_error.lock().unwrap() = Some(line.clone());
            }

            // 写日志环
            {
                let mut g = log.lock().unwrap();
                if g.len() >= MAX_LOG_LINES {
                    g.remove(0);
                }
                g.push(line.clone());
            }

            // 推送给前端
            let _ = app.emit(
                "proxy-log",
                LogEvent {
                    proxy_id: proxy_id.clone(),
                    line,
                },
            );
        }
        // 流结束：进程已退出
        let mut s = status.lock().unwrap();
        if !matches!(*s, ProxyRunStatus::Stopped) {
            *s = ProxyRunStatus::Crashed;
        }
        let _ = app.emit(
            "proxy-status",
            StatusEvent {
                proxy_id: proxy_id.clone(),
                status: s.clone(),
            },
        );
    });
}

#[derive(Serialize, Clone)]
struct LogEvent {
    proxy_id: String,
    line: String,
}

#[derive(Serialize, Clone)]
struct StatusEvent {
    proxy_id: String,
    status: ProxyRunStatus,
}

// ---------- commands ----------

#[tauri::command]
pub fn list_runtime(
    runtime: tauri::State<Runtime>,
) -> Vec<ProxyRuntime> {
    let g = runtime.procs.lock().unwrap();
    g.iter()
        .map(|(id, p)| ProxyRuntime {
            proxy_id: id.clone(),
            status: p.status.lock().unwrap().clone(),
            pid: Some(p.child.id()),
            last_error: p.last_error.lock().unwrap().clone(),
            log_tail: p.log.lock().unwrap().clone(),
        })
        .collect()
}

#[tauri::command]
pub fn start_proxy(
    app: AppHandle,
    store: tauri::State<Store>,
    runtime: tauri::State<Runtime>,
    proxy_id: String,
) -> Result<ProxyRuntime, String> {
    let snap = store.snapshot();
    let proxy = snap
        .proxies
        .iter()
        .find(|p| p.id == proxy_id)
        .cloned()
        .ok_or_else(|| "proxy 不存在".to_string())?;
    let server = snap
        .servers
        .iter()
        .find(|s| s.id == proxy.server_id)
        .cloned()
        .ok_or_else(|| "服务端不存在".to_string())?;

    // 已在跑
    {
        let g = runtime.procs.lock().unwrap();
        if g.contains_key(&proxy_id) {
            return Err("该 proxy 已在运行".into());
        }
    }

    let bin = frpc_path(&app)?;
    let cfg = write_config(&app, &proxy, &server)?;
    let mut cmd = Command::new(&bin);
    cmd.arg("-c").arg(&cfg);
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());

    let mut child = cmd.spawn().map_err(|e| format!("启动 frpc 失败: {e}"))?;

    let log = Arc::new(Mutex::new(Vec::<String>::new()));
    let status = Arc::new(Mutex::new(ProxyRunStatus::Starting));
    let last_error = Arc::new(Mutex::new(None::<String>));

    if let Some(out) = child.stdout.take() {
        spawn_log_pump(out, log.clone(), status.clone(), last_error.clone(), app.clone(), proxy_id.clone());
    }
    if let Some(err) = child.stderr.take() {
        spawn_log_pump(err, log.clone(), status.clone(), last_error.clone(), app.clone(), proxy_id.clone());
    }

    let pid = child.id();
    let log_snapshot = log.lock().unwrap().clone();
    {
        let mut g = runtime.procs.lock().unwrap();
        g.insert(
            proxy_id.clone(),
            RunningProc {
                child,
                log: log.clone(),
                status: status.clone(),
                last_error: last_error.clone(),
            },
        );
    }

    Ok(ProxyRuntime {
        proxy_id,
        status: ProxyRunStatus::Starting,
        pid: Some(pid),
        last_error: None,
        log_tail: log_snapshot,
    })
}

#[tauri::command]
pub fn stop_proxy(
    runtime: tauri::State<Runtime>,
    proxy_id: String,
) -> Result<(), String> {
    let mut g = runtime.procs.lock().unwrap();
    if let Some(mut p) = g.remove(&proxy_id) {
        *p.status.lock().unwrap() = ProxyRunStatus::Stopped;
        let _ = p.child.kill();
        let _ = p.child.wait();
    }
    Ok(())
}

#[tauri::command]
pub fn proxy_logs(
    runtime: tauri::State<Runtime>,
    proxy_id: String,
) -> Vec<String> {
    let g = runtime.procs.lock().unwrap();
    g.get(&proxy_id)
        .map(|p| p.log.lock().unwrap().clone())
        .unwrap_or_default()
}

#[tauri::command]
pub fn check_frpc(app: AppHandle) -> Result<String, String> {
    let bin = frpc_path(&app)?;
    let out = Command::new(&bin)
        .arg("--version")
        .output()
        .map_err(|e| format!("调用 frpc 失败: {e}"))?;
    let v = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if v.is_empty() {
        Ok(format!("frpc 在 {} (版本读取失败)", bin.display()))
    } else {
        Ok(format!("{v} ({})", bin.display()))
    }
}

// 在应用退出前调用，确保子进程被清理
pub fn shutdown_all(runtime: &Runtime) {
    let mut g = runtime.procs.lock().unwrap();
    for (_, mut p) in g.drain() {
        let _ = p.child.kill();
    }
}

// 不直接用 Path 但保留以备将来
#[allow(dead_code)]
fn _ensure_path<P: AsRef<Path>>(_p: P) {}
