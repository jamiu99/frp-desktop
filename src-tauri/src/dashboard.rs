use std::{collections::BTreeMap, time::Duration};

use chrono::{DateTime, Utc};
use futures_util::future::join_all;
use reqwest::{Client, Url};
use serde::{de::DeserializeOwned, Deserialize, Deserializer, Serialize};
use tauri::State;

use crate::store::{FrpsServer, Store};

const PROXY_TYPES: &[&str] = &[
    "tcp", "udp", "http", "https", "stcp", "sudp", "xtcp", "tcpmux",
];

#[derive(Deserialize, Serialize)]
pub struct ServerInfo {
    version: String,
    #[serde(alias = "bindPort")]
    bind_port: Option<u16>,
    #[serde(alias = "vhostHTTPPort")]
    vhost_http_port: Option<u16>,
    #[serde(alias = "vhostHTTPSPort")]
    vhost_https_port: Option<u16>,
    #[serde(alias = "curConns")]
    cur_conns: Option<u64>,
    #[serde(alias = "clientCounts")]
    client_counts: Option<u64>,
    #[serde(alias = "totalTrafficIn")]
    total_traffic_in: Option<u64>,
    #[serde(alias = "totalTrafficOut")]
    total_traffic_out: Option<u64>,
    #[serde(alias = "proxyTypeCount")]
    proxy_type_count: Option<BTreeMap<String, u64>>,
}

#[derive(Deserialize, Serialize)]
pub struct ProxyEndpoint {
    #[serde(alias = "remotePort")]
    remote_port: Option<u16>,
    #[serde(alias = "customDomains")]
    custom_domains: Option<Vec<String>>,
}

#[derive(Deserialize, Serialize)]
pub struct RemoteProxy {
    pub name: String,
    pub status: String,
    // Only these endpoint fields are needed. frps may include more in `conf`.
    conf: Option<ProxyEndpoint>,
    #[serde(alias = "curConns")]
    cur_conns: Option<u64>,
    #[serde(alias = "todayTrafficIn")]
    today_traffic_in: Option<u64>,
    #[serde(alias = "todayTrafficOut")]
    today_traffic_out: Option<u64>,
    #[serde(alias = "lastStartTime")]
    last_start_time: Option<String>,
    #[serde(alias = "lastCloseTime")]
    last_close_time: Option<String>,
}

#[derive(Deserialize)]
struct ProxyList {
    #[serde(deserialize_with = "deserialize_proxies")]
    proxies: Vec<RemoteProxy>,
}

fn deserialize_proxies<'de, D: Deserializer<'de>>(d: D) -> Result<Vec<RemoteProxy>, D::Error> {
    Option::<Vec<RemoteProxy>>::deserialize(d).map(Option::unwrap_or_default)
}

#[derive(Serialize)]
struct TypedProxy {
    #[serde(rename = "type")]
    proxy_type: &'static str,
    #[serde(flatten)]
    proxy: RemoteProxy,
}

#[derive(Serialize)]
pub struct DashboardSnapshot {
    observed_at: DateTime<Utc>,
    server_info: Option<ServerInfo>,
    server_info_error: Option<String>,
    proxies: Vec<TypedProxy>,
    failed_types: BTreeMap<&'static str, String>,
}

impl DashboardSnapshot {
    pub fn status(&self) -> &'static str {
        match (self.server_info.is_some(), self.failed_types.len()) {
            (true, 0) => "available",
            (false, failed) if failed == PROXY_TYPES.len() => "unavailable",
            _ => "partial",
        }
    }
}

pub struct DashboardClient(Client);

impl DashboardClient {
    pub fn new() -> Result<Self, reqwest::Error> {
        // Share the TLS provider used by the desktop updater.
        let _ = rustls::crypto::ring::default_provider().install_default();
        Client::builder()
            .connect_timeout(Duration::from_secs(3))
            .timeout(Duration::from_secs(6))
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .map(Self)
    }

    pub async fn snapshot(&self, server: &FrpsServer) -> Result<DashboardSnapshot, String> {
        let dashboard = self.dashboard(server)?;
        let dashboard = &dashboard;
        let (info, results) = tokio::join!(
            dashboard.get(&["api", "serverinfo"]),
            join_all(
                PROXY_TYPES
                    .iter()
                    .map(|&ty| async move { (ty, dashboard.proxies(ty).await) })
            ),
        );

        let (server_info, server_info_error) = match info {
            Ok(info) => (Some(info), None),
            Err(error) => (None, Some(error)),
        };
        let mut proxies = Vec::new();
        let mut failed_types = BTreeMap::new();
        for (proxy_type, result) in results {
            match result {
                Ok(items) => proxies.extend(
                    items
                        .into_iter()
                        .map(|proxy| TypedProxy { proxy_type, proxy }),
                ),
                Err(error) => {
                    failed_types.insert(proxy_type, error);
                }
            }
        }
        Ok(DashboardSnapshot {
            observed_at: Utc::now(),
            server_info,
            server_info_error,
            proxies,
            failed_types,
        })
    }

    pub async fn proxy(
        &self,
        server: &FrpsServer,
        ty: &str,
        name: &str,
    ) -> Result<Option<RemoteProxy>, String> {
        let proxies = self.dashboard(server)?.proxies(ty).await?;
        Ok(proxies.into_iter().find(|proxy| proxy.name == name))
    }

    fn dashboard<'a>(&'a self, server: &'a FrpsServer) -> Result<Dashboard<'a>, String> {
        let url = server
            .dashboard_url
            .as_deref()
            .filter(|s| !s.trim().is_empty())
            .ok_or("该服务端未配置 Dashboard URL")?;
        Ok(Dashboard {
            client: &self.0,
            base_url: parse_url(url)?,
            server,
        })
    }
}

pub fn parse_url(url: &str) -> Result<Url, String> {
    let url = Url::parse(url.trim()).map_err(|_| "Dashboard URL 无效".to_string())?;
    if !matches!(url.scheme(), "http" | "https")
        || url.host().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return Err("Dashboard URL 须使用 HTTP 或 HTTPS，且不能包含凭证、查询参数或片段".into());
    }
    Ok(url)
}

struct Dashboard<'a> {
    client: &'a Client,
    base_url: Url,
    server: &'a FrpsServer,
}

impl Dashboard<'_> {
    async fn get<T: DeserializeOwned>(&self, path: &[&str]) -> Result<T, String> {
        let mut url = self.base_url.clone();
        url.path_segments_mut()
            .map_err(|_| "Dashboard URL 无效")?
            .pop_if_empty()
            .extend(path);
        let mut request = self.client.get(url);
        if let (Some(user), Some(password)) =
            (&self.server.dashboard_user, &self.server.dashboard_pass)
        {
            request = request.basic_auth(user, Some(password));
        }
        let response = request.send().await.map_err(|e| {
            if e.is_timeout() {
                "Dashboard 请求超时"
            } else {
                "无法连接 Dashboard"
            }
        })?;
        if !response.status().is_success() {
            return Err(format!(
                "Dashboard 返回 HTTP {}",
                response.status().as_u16()
            ));
        }
        response.json().await.map_err(|e| {
            if e.is_timeout() {
                "Dashboard 请求超时"
            } else {
                "Dashboard 返回的数据格式无效"
            }
            .into()
        })
    }

    async fn proxies(&self, ty: &str) -> Result<Vec<RemoteProxy>, String> {
        self.get::<ProxyList>(&["api", "proxy", ty])
            .await
            .map(|r| r.proxies)
    }
}

#[tauri::command]
pub async fn dashboard_overview(
    store: State<'_, Store>,
    dashboard: State<'_, DashboardClient>,
    server_id: String,
) -> Result<DashboardSnapshot, String> {
    let server = store
        .snapshot()
        .servers
        .into_iter()
        .find(|s| s.id == server_id)
        .ok_or("服务端不存在")?;
    dashboard.snapshot(&server).await
}

#[cfg(test)]
mod tests {
    use tokio::{
        io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
        net::{TcpListener, TcpStream},
    };

    use super::*;

    #[tokio::test]
    async fn dashboard_returns_available_fields_and_reports_unqueried_types() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let mock = tokio::spawn(async move {
            while let Ok((peer, _)) = listener.accept().await {
                tokio::spawn(mock_dashboard(peer));
            }
        });
        let dashboard = DashboardClient::new().unwrap();
        let server = FrpsServer {
            id: "fixture".into(),
            name: "fixture".into(),
            host: "example.invalid".into(),
            port: 7000,
            token: String::new(),
            dashboard_url: Some(format!("http://{address}/dashboard/")),
            dashboard_user: Some("user".into()),
            dashboard_pass: Some("pw".into()),
            created_at: Utc::now(),
        };
        let snapshot = dashboard.snapshot(&server).await.unwrap();
        assert_eq!(snapshot.status(), "partial");
        assert_eq!(
            snapshot.failed_types.get("udp").unwrap(),
            "Dashboard 返回 HTTP 503"
        );
        assert_eq!(snapshot.failed_types.len(), 1);
        assert_eq!(snapshot.server_info.unwrap().client_counts, Some(2));
        assert_eq!(
            snapshot.proxies[0].proxy.conf.as_ref().unwrap().remote_port,
            Some(18080)
        );
        assert!(!serde_json::to_string(&snapshot.proxies)
            .unwrap()
            .contains("fixture-extra-field"));
        assert_eq!(
            dashboard
                .proxy(&server, "tcp", "web")
                .await
                .unwrap()
                .unwrap()
                .status,
            "online"
        );
        assert!(dashboard
            .proxy(&server, "tcp", "unlisted")
            .await
            .unwrap()
            .is_none());
        assert!(dashboard.proxy(&server, "udp", "web").await.is_err());
        mock.abort();
        let _ = mock.await;
    }

    async fn mock_dashboard(mut socket: TcpStream) {
        let mut reader = BufReader::new(&mut socket);
        let mut request = String::new();
        reader.read_line(&mut request).await.unwrap();
        let mut authorized = false;
        loop {
            let mut line = String::new();
            reader.read_line(&mut line).await.unwrap();
            if line == "\r\n" || line.is_empty() {
                break;
            }
            authorized |= line
                .trim_end()
                .eq_ignore_ascii_case("authorization: Basic dXNlcjpwdw==");
        }
        let path = request.split_whitespace().nth(1).unwrap_or("");
        let (status, body) = if !authorized {
            ("401 Unauthorized", "{}")
        } else {
            match path {
                "/dashboard/api/serverinfo" => {
                    ("200 OK", r#"{"version":"fixture","clientCounts":2}"#)
                }
                "/dashboard/api/proxy/tcp" => (
                    "200 OK",
                    r#"{"proxies":[{"name":"web","status":"online","conf":{"remotePort":18080,"secretKey":"fixture-extra-field"}}]}"#,
                ),
                "/dashboard/api/proxy/udp" => ("503 Service Unavailable", "{}"),
                _ => ("200 OK", r#"{"proxies":null}"#),
            }
        };
        let response = format!("HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len());
        socket.write_all(response.as_bytes()).await.unwrap();
    }
}
