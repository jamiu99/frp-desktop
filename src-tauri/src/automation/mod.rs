mod cli;
mod transport;

use std::process::ExitCode;
use std::sync::Arc;

use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tauri::{AppHandle, Emitter, Manager};
use tokio::sync::{oneshot, Semaphore};

use crate::{
    dashboard::{self, DashboardClient},
    frpc::{self, Runtime, MAX_LOG_LINES},
    store::{self, FrpsServer, Proxy, ProxyInput, ServerInput, Store, StoreData},
};

#[derive(Serialize, Deserialize)]
#[serde(tag = "command", rename_all = "snake_case")]
enum Request {
    ServerList,
    ServerAdd {
        input: ServerInput,
    },
    ServerStatus {
        server: String,
    },
    ProxyList {
        server: Option<String>,
    },
    ProxyCreate {
        server: String,
        input: ProxyInput,
        dry_run: bool,
    },
    ProxyStatus {
        server: String,
        proxy: String,
    },
    ProxyChange {
        server: String,
        proxy: String,
        enabled: bool,
        dry_run: bool,
    },
    Logs {
        server: String,
        lines: usize,
    },
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
enum Reply {
    Ok { data: Value },
    Error { error: String },
}

impl From<Result<Value, String>> for Reply {
    fn from(result: Result<Value, String>) -> Self {
        match result {
            Ok(data) => Self::Ok { data },
            Err(error) => Self::Error { error },
        }
    }
}

pub fn run_cli() -> ExitCode {
    cli::run()
}

pub fn start(app: AppHandle) -> tauri::async_runtime::JoinHandle<()> {
    tauri::async_runtime::spawn(async move {
        if let Err(error) = serve(app).await {
            eprintln!("[frp-desktop-cli] 本机接口无法运行：{error}");
        }
    })
}

async fn serve(app: AppHandle) -> Result<(), Box<dyn std::error::Error>> {
    let listener = transport::LocalListener::bind(&app.path().app_data_dir()?).await?;
    let slots = Arc::new(Semaphore::new(8));
    loop {
        let slot = slots.clone().acquire_owned().await?;
        let mut connection = listener.accept().await?;
        let app = app.clone();
        tokio::spawn(async move {
            let _slot = slot;
            if let Ok(request) = connection.request().await {
                let reply = handle(app, request).await.into();
                let _ = connection.reply(&reply).await;
            }
        });
    }
}

enum LocalResult {
    Done(Value),
    ServerStatus(FrpsServer, Value),
    ProxyStatus(FrpsServer, Proxy, Value),
}

async fn handle(app: AppHandle, request: Request) -> Result<Value, String> {
    // Use the same event loop as the UI for store changes and frpc operations.
    let (sender, receiver) = oneshot::channel();
    let handler = app.clone();
    app.run_on_main_thread(move || {
        let _ = sender.send(handle_local(&handler, request));
    })
    .map_err(|_| "FRP Desktop 正在退出")?;
    let local = receiver.await.map_err(|_| "FRP Desktop 正在退出")??;
    let dashboard = app.state::<DashboardClient>();
    match local {
        LocalResult::Done(result) => Ok(result),
        LocalResult::ServerStatus(server, mut result) => {
            result["dashboard"] = if !dashboard_configured(&server) {
                json!({"status": "unconfigured"})
            } else {
                match dashboard.snapshot(&server).await {
                    Ok(snapshot) => json!({ "status": snapshot.status(), "snapshot": snapshot }),
                    Err(error) => json!({ "status": "unavailable", "error": error }),
                }
            };
            Ok(result)
        }
        LocalResult::ProxyStatus(server, proxy, mut result) => {
            let remote = if !dashboard_configured(&server) {
                json!({"status": "unknown", "reason": "dashboard_unconfigured"})
            } else {
                match dashboard
                    .proxy(&server, proxy.proxy_type.as_str(), &proxy.name)
                    .await
                {
                    Ok(Some(entry)) => {
                        let status = match entry.status.as_str() {
                            "online" => "online",
                            "offline" => "offline",
                            _ => "unknown",
                        };
                        json!({"status": status, "observed_at": Utc::now(), "proxy": entry})
                    }
                    Ok(None) => json!({"status": "not_listed", "observed_at": Utc::now()}),
                    Err(error) => json!({"status": "unknown", "error": error}),
                }
            };
            result["remote"] = remote;
            Ok(result)
        }
    }
}

fn handle_local(app: &AppHandle, request: Request) -> Result<LocalResult, String> {
    let store = app.state::<Store>();
    let runtime = app.state::<Runtime>();
    let data = store.snapshot();
    let result = match request {
        Request::ServerList => Value::Array(data.servers.iter().map(server_summary).collect()),
        Request::ServerAdd { input } => {
            if let Some(url) = &input.dashboard_url {
                dashboard::parse_url(url)?;
            }
            let server = store::add_server(&store, input)?;
            let _ = app.emit("store-changed", ());
            server_summary(&server)
        }
        Request::ServerStatus { server } => {
            let server = find_server(&data, &server)?;
            let local = runtime.snapshot(&server.id, 0);
            let value = json!({ "server": server_summary(server), "local": local });
            return Ok(LocalResult::ServerStatus(server.clone(), value));
        }
        Request::ProxyList { server } => {
            let server = server
                .as_deref()
                .map(|s| find_server(&data, s))
                .transpose()?;
            let proxies: Vec<_> = data
                .proxies
                .iter()
                .filter(|p| server.is_none_or(|s| p.server_id == s.id))
                .collect();
            json!(proxies)
        }
        Request::ProxyCreate {
            server,
            mut input,
            dry_run,
        } => {
            let server = find_server(&data, &server)?;
            input.server_id = server.id.clone();
            if dry_run {
                store::validate_new_proxy(&input, &data)?;
                json!({"dry_run": true, "enabled": false, "server": server_summary(server), "proxy": input})
            } else {
                let proxy = store::add_proxy(&store, input)?;
                let _ = app.emit("store-changed", ());
                json!({"dry_run": false, "enabled": false, "server": server_summary(server), "proxy": proxy})
            }
        }
        Request::ProxyStatus { server, proxy } => {
            let server = find_server(&data, &server)?;
            let proxy = find_proxy(&data, server, &proxy)?;
            let value = json!({
                "server": server_summary(server), "proxy": proxy,
                "local": runtime.snapshot(&server.id, 0),
                "frpc_includes_proxy": runtime.includes_proxy(&server.id, &proxy.id),
            });
            return Ok(LocalResult::ProxyStatus(
                server.clone(),
                proxy.clone(),
                value,
            ));
        }
        Request::ProxyChange {
            server,
            proxy,
            enabled,
            dry_run,
        } => {
            let server = find_server(&data, &server)?;
            let proxy = find_proxy(&data, server, &proxy)?;
            let result = frpc::change_proxy(app, &store, &runtime, &proxy.id, enabled, dry_run);
            if !dry_run {
                let _ = app.emit("store-changed", ());
            }
            json!(result?)
        }
        Request::Logs { server, lines } => {
            if !(1..=MAX_LOG_LINES).contains(&lines) {
                return Err(format!("日志行数须为 1 到 {MAX_LOG_LINES}"));
            }
            let server = find_server(&data, &server)?;
            json!({"server": server_summary(server), "local": runtime.snapshot(&server.id, lines)})
        }
    };
    Ok(LocalResult::Done(result))
}

fn find_server<'a>(data: &'a StoreData, query: &str) -> Result<&'a FrpsServer, String> {
    data.servers
        .iter()
        .find(|s| s.id == query)
        .or_else(|| data.servers.iter().find(|s| s.name == query))
        .ok_or_else(|| "找不到该 frps 服务端，可用 server list 查询".into())
}

fn find_proxy<'a>(
    data: &'a StoreData,
    server: &FrpsServer,
    query: &str,
) -> Result<&'a Proxy, String> {
    let mut proxies = data.proxies.iter().filter(|p| p.server_id == server.id);
    proxies
        .clone()
        .find(|p| p.id == query)
        .or_else(|| proxies.find(|p| p.name == query))
        .ok_or_else(|| "该服务端下找不到这个 proxy，可用 proxy list 查询".into())
}

fn server_summary(server: &FrpsServer) -> Value {
    json!({"id": server.id, "name": server.name, "host": server.host, "port": server.port,
        "dashboard_configured": dashboard_configured(server)})
}

fn dashboard_configured(server: &FrpsServer) -> bool {
    server
        .dashboard_url
        .as_deref()
        .is_some_and(|s| !s.trim().is_empty())
}
