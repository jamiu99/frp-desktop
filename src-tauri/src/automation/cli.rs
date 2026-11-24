use std::{
    io::{self, IsTerminal, Read, Write},
    process::ExitCode,
};

use clap::{Args, Parser, Subcommand, ValueEnum};
use serde::Deserialize;

use crate::store::{ProxyInput, ProxyType, ServerInput};

use super::{transport, Reply, Request};

#[derive(Parser)]
#[command(
    name = "frp-desktop-cli",
    version,
    about = "通过运行中的 FRP Desktop 查询和管理 frps 与 proxy"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// 管理 frps 服务端
    Server {
        #[command(subcommand)]
        command: Server,
    },
    /// 管理本机 proxy
    Proxy {
        #[command(subcommand)]
        command: Proxy,
    },
    /// 查看此 frps 对应的本地 frpc 日志
    Logs {
        /// 服务端名称或 ID
        server: String,
        #[arg(long, default_value_t = 50, value_parser = clap::value_parser!(u16).range(1..=500))]
        lines: u16,
    },
}

#[derive(Subcommand)]
enum Server {
    /// 列出已配置的服务端
    List,
    /// 保存新服务端配置
    Add {
        #[arg(long)]
        name: String,
        #[arg(long)]
        host: String,
        #[arg(long)]
        port: u16,
        #[arg(long)]
        dashboard_url: Option<String>,
        /// 从标准输入读取凭证 JSON，字段为 token、dashboard_user、dashboard_pass
        #[arg(long)]
        credentials_stdin: bool,
    },
    /// 查看本机 frpc 与远端 Dashboard 状态
    Status {
        /// 服务端名称或 ID
        server: String,
    },
}

#[derive(Subcommand)]
enum Proxy {
    /// 列出本机 proxy 配置
    List {
        /// 按服务端名称或 ID 过滤
        #[arg(long)]
        server: Option<String>,
    },
    /// 校验或创建一个处于停用状态的 proxy
    Create {
        /// 服务端名称或 ID
        #[arg(long)]
        server: String,
        #[arg(long)]
        name: String,
        #[arg(long = "type", value_enum)]
        proxy_type: CreateProxyType,
        #[arg(long, default_value = "127.0.0.1")]
        local_ip: String,
        #[arg(long)]
        local_port: u16,
        #[arg(long)]
        remote_port: Option<u16>,
        /// HTTP/HTTPS 域名，可重复指定
        #[arg(long = "domain")]
        custom_domains: Vec<String>,
        #[arg(long, default_value = "")]
        description: String,
        /// 仅校验并显示拟创建的配置
        #[arg(long)]
        dry_run: bool,
    },
    /// 查看此 proxy 的配置、本地 frpc 和远端状态
    Status(ProxyTarget),
    /// 启用 proxy，可先预览影响
    Start {
        #[command(flatten)]
        target: ProxyTarget,
        #[arg(long)]
        dry_run: bool,
    },
    /// 停用 proxy，可先预览影响
    Stop {
        #[command(flatten)]
        target: ProxyTarget,
        #[arg(long)]
        dry_run: bool,
    },
}

#[derive(Args)]
struct ProxyTarget {
    /// 服务端名称或 ID
    #[arg(long)]
    server: String,
    /// Proxy 名称或 ID
    proxy: String,
}

#[derive(Clone, Copy, ValueEnum)]
enum CreateProxyType {
    Tcp,
    Udp,
    Http,
    Https,
}

impl From<CreateProxyType> for ProxyType {
    fn from(value: CreateProxyType) -> Self {
        match value {
            CreateProxyType::Tcp => Self::Tcp,
            CreateProxyType::Udp => Self::Udp,
            CreateProxyType::Http => Self::Http,
            CreateProxyType::Https => Self::Https,
        }
    }
}

#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct Credentials {
    #[serde(default)]
    token: String,
    dashboard_user: Option<String>,
    dashboard_pass: Option<String>,
}

impl Credentials {
    fn read_stdin() -> Result<Self, String> {
        let stdin = io::stdin();
        if stdin.is_terminal() {
            return Err("请通过标准输入重定向凭证 JSON，或在桌面应用中录入凭证".into());
        }
        let mut payload = Vec::new();
        stdin
            .take((transport::MAX_REQUEST_BYTES + 1) as u64)
            .read_to_end(&mut payload)
            .map_err(|_| "读取凭证失败")?;
        if payload.len() > transport::MAX_REQUEST_BYTES {
            return Err("凭证 JSON 过长".into());
        }
        serde_json::from_slice(&payload).map_err(|_| "凭证 JSON 格式无效".into())
    }
}

impl Command {
    fn request(self) -> Result<Request, String> {
        Ok(match self {
            Self::Server { command } => match command {
                Server::List => Request::ServerList,
                Server::Status { server } => Request::ServerStatus { server },
                Server::Add {
                    name,
                    host,
                    port,
                    dashboard_url,
                    credentials_stdin,
                } => {
                    let credentials = if credentials_stdin {
                        Credentials::read_stdin()?
                    } else {
                        Credentials::default()
                    };
                    Request::ServerAdd {
                        input: ServerInput {
                            name,
                            host,
                            port,
                            token: credentials.token,
                            dashboard_url,
                            dashboard_user: credentials.dashboard_user,
                            dashboard_pass: credentials.dashboard_pass,
                        },
                    }
                }
            },
            Self::Proxy { command } => match command {
                Proxy::List { server } => Request::ProxyList { server },
                Proxy::Create {
                    server,
                    name,
                    proxy_type,
                    local_ip,
                    local_port,
                    remote_port,
                    custom_domains,
                    description,
                    dry_run,
                } => Request::ProxyCreate {
                    server,
                    input: ProxyInput {
                        server_id: String::new(),
                        name,
                        description,
                        proxy_type: proxy_type.into(),
                        local_ip,
                        local_port,
                        remote_port,
                        custom_domains,
                    },
                    dry_run,
                },
                Proxy::Status(ProxyTarget { server, proxy }) => {
                    Request::ProxyStatus { server, proxy }
                }
                Proxy::Start {
                    target: ProxyTarget { server, proxy },
                    dry_run,
                } => Request::ProxyChange {
                    server,
                    proxy,
                    enabled: true,
                    dry_run,
                },
                Proxy::Stop {
                    target: ProxyTarget { server, proxy },
                    dry_run,
                } => Request::ProxyChange {
                    server,
                    proxy,
                    enabled: false,
                    dry_run,
                },
            },
            Self::Logs { server, lines } => Request::Logs {
                server,
                lines: usize::from(lines),
            },
        })
    }
}

#[tokio::main(flavor = "current_thread")]
pub async fn run() -> ExitCode {
    let reply = match Cli::parse().command.request() {
        Ok(request) => match transport::call(&request).await {
            Ok(reply) => reply,
            Err(error) => Reply::Error { error },
        },
        Err(error) => Reply::Error { error },
    };
    let stdout = io::stdout();
    let mut output = stdout.lock();
    if serde_json::to_writer_pretty(&mut output, &reply).is_err() || writeln!(output).is_err() {
        return ExitCode::FAILURE;
    }
    match reply {
        Reply::Ok { .. } => ExitCode::SUCCESS,
        Reply::Error { .. } => ExitCode::FAILURE,
    }
}
