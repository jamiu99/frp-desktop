//! 数据存储：单 JSON 文件，存在 app_data_dir/store.json
//!
//! - 单文件读全量、改全量、写全量；写入用 tmp + rename 保证原子性
//! - token / dashboard 密码明文存（README 已说明），文件权限默认靠 OS 用户目录隔离
//!
//! v0.1.1 起去掉 Project 概念：proxy 直接挂在 frps 服务端下；
//! 描述/简介允许空；name 用户自填，只校验格式 + 同 server 内唯一。

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use tauri::{AppHandle, Manager};
use uuid::Uuid;

// ---------- 数据模型 ----------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FrpsServer {
    pub id: String,
    pub name: String,
    pub host: String,
    pub port: u16,
    pub token: String,
    pub dashboard_url: Option<String>,
    pub dashboard_user: Option<String>,
    pub dashboard_pass: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum ProxyType {
    Tcp,
    Udp,
    Http,
    Https,
    Stcp,
}

impl ProxyType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Tcp => "tcp",
            Self::Udp => "udp",
            Self::Http => "http",
            Self::Https => "https",
            Self::Stcp => "stcp",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Proxy {
    pub id: String,
    pub server_id: String,
    /// frpc 配置中的 proxy name
    pub name: String,
    /// 可选的备注/描述
    #[serde(default)]
    pub description: String,
    pub proxy_type: ProxyType,
    pub local_ip: String,
    pub local_port: u16,
    pub remote_port: Option<u16>,
    #[serde(default)]
    pub custom_domains: Vec<String>,
    #[serde(default)]
    pub enabled: bool,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Settings {
    /// 关闭主窗口时是否最小化到托盘
    #[serde(default)]
    pub close_to_tray: bool,
    /// 开机自启
    #[serde(default)]
    pub autostart: bool,
    /// 自定义 frpc 二进制路径（None 表示用打包内置的）
    #[serde(default)]
    pub frpc_path: Option<String>,
    /// Windows: 是否显示 frpc 控制台窗口（默认隐藏）
    #[serde(default)]
    pub show_frpc_console: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct StoreData {
    /// v0.1.0 兼容：旧文件可能有 projects 字段，反序列化时忽略它
    #[serde(default, skip_serializing)]
    #[allow(dead_code)]
    pub projects: serde_json::Value,
    #[serde(default)]
    pub servers: Vec<FrpsServer>,
    #[serde(default)]
    pub proxies: Vec<Proxy>,
    #[serde(default)]
    pub settings: Settings,
}

// ---------- Store ----------

pub struct Store {
    path: PathBuf,
    data: Mutex<StoreData>,
}

impl Store {
    pub fn load(app: &AppHandle) -> Result<Self, String> {
        let dir = app
            .path()
            .app_data_dir()
            .map_err(|e| format!("get app_data_dir: {e}"))?;
        Self::open(&dir)
    }

    fn open(dir: &Path) -> Result<Self, String> {
        fs::create_dir_all(dir).map_err(|e| format!("create dir: {e}"))?;
        let path = dir.join("store.json");

        let data: StoreData = if path.exists() {
            let txt = fs::read_to_string(&path).map_err(|e| format!("read store: {e}"))?;
            // 旧文件中的 proxy 可能含 project_id 字段——serde 默认忽略未知字段
            serde_json::from_str(&txt).map_err(|e| format!("parse store: {e}"))?
        } else {
            StoreData::default()
        };

        Ok(Self {
            path,
            data: Mutex::new(data),
        })
    }

    fn persist(&self, data: &StoreData) -> Result<(), String> {
        let mut tmp = tempfile::NamedTempFile::new_in(self.path.parent().unwrap())
            .map_err(|e| format!("create temp store: {e}"))?;
        serde_json::to_writer_pretty(tmp.as_file_mut(), data)
            .map_err(|e| format!("write store: {e}"))?;
        tmp.persist(&self.path)
            .map_err(|e| format!("save store: {e}"))?;
        Ok(())
    }

    pub fn snapshot(&self) -> StoreData {
        self.data.lock().unwrap().clone()
    }

    pub fn mutate<T>(
        &self,
        f: impl FnOnce(&mut StoreData) -> Result<T, String>,
    ) -> Result<T, String> {
        let mut guard = self.data.lock().unwrap();
        let mut updated = guard.clone();
        let result = f(&mut updated)?;
        self.persist(&updated)?;
        *guard = updated;
        Ok(result)
    }
}

// ---------- 校验 ----------

const NAME_RE: &str = r"^[A-Za-z0-9_][A-Za-z0-9_.-]{0,63}$";

fn is_valid_name(s: &str) -> bool {
    regex_lite::Regex::new(NAME_RE).unwrap().is_match(s)
}

pub fn validate_proxy_name(name: &str) -> Result<(), String> {
    if name.trim().is_empty() {
        return Err("name 不能为空".into());
    }
    if !is_valid_name(name) {
        return Err("name 只能字母/数字/下划线/点/连字符，最多 64 字符".into());
    }
    Ok(())
}

// ---------- Tauri Commands ----------

#[tauri::command]
pub fn get_state(store: tauri::State<Store>) -> StoreData {
    store.snapshot()
}

// --- Server ---

#[derive(Serialize, Deserialize)]
pub struct ServerInput {
    pub name: String,
    pub host: String,
    pub port: u16,
    pub token: String,
    pub dashboard_url: Option<String>,
    pub dashboard_user: Option<String>,
    pub dashboard_pass: Option<String>,
}

fn validate_server_input(s: &ServerInput) -> Result<(), String> {
    if s.name.trim().is_empty() {
        return Err("服务端名称不能为空".into());
    }
    if s.host.trim().is_empty() {
        return Err("frps host 不能为空".into());
    }
    if s.port == 0 {
        return Err("frps port 不合法".into());
    }
    Ok(())
}

#[tauri::command]
pub fn create_server(store: tauri::State<Store>, input: ServerInput) -> Result<FrpsServer, String> {
    add_server(&store, input)
}

pub fn add_server(store: &Store, input: ServerInput) -> Result<FrpsServer, String> {
    validate_server_input(&input)?;
    store.mutate(|d| {
        if d.servers.iter().any(|s| s.name == input.name) {
            return Err(format!("服务端名称已存在：{}", input.name));
        }
        let server = FrpsServer {
            id: Uuid::new_v4().to_string(),
            name: input.name,
            host: input.host,
            port: input.port,
            token: input.token,
            dashboard_url: input.dashboard_url,
            dashboard_user: input.dashboard_user,
            dashboard_pass: input.dashboard_pass,
            created_at: Utc::now(),
        };
        d.servers.push(server.clone());
        Ok(server)
    })
}

#[tauri::command]
pub fn update_server(
    store: tauri::State<Store>,
    id: String,
    input: ServerInput,
) -> Result<FrpsServer, String> {
    validate_server_input(&input)?;
    store.mutate(|d| {
        if d.servers.iter().any(|s| s.id != id && s.name == input.name) {
            return Err(format!("服务端名称已存在：{}", input.name));
        }
        let s = d
            .servers
            .iter_mut()
            .find(|s| s.id == id)
            .ok_or_else(|| format!("服务端不存在：{id}"))?;
        s.name = input.name.clone();
        s.host = input.host.clone();
        s.port = input.port;
        s.token = input.token.clone();
        s.dashboard_url = input.dashboard_url.clone();
        s.dashboard_user = input.dashboard_user.clone();
        s.dashboard_pass = input.dashboard_pass.clone();
        Ok(())
    })?;
    store
        .snapshot()
        .servers
        .into_iter()
        .find(|s| s.id == id)
        .ok_or_else(|| "更新后找不到".into())
}

#[tauri::command]
pub fn delete_server(store: tauri::State<Store>, id: String) -> Result<(), String> {
    store.mutate(|d| {
        d.proxies.retain(|p| p.server_id != id);
        d.servers.retain(|s| s.id != id);
        Ok(())
    })?;
    Ok(())
}

// --- Proxy ---

#[derive(Serialize, Deserialize)]
pub struct ProxyInput {
    pub server_id: String,
    pub name: String,
    #[serde(default)]
    pub description: String,
    pub proxy_type: ProxyType,
    pub local_ip: String,
    pub local_port: u16,
    pub remote_port: Option<u16>,
    #[serde(default)]
    pub custom_domains: Vec<String>,
}

fn validate_proxy_input(input: &ProxyInput, data: &StoreData) -> Result<(), String> {
    if !data.servers.iter().any(|s| s.id == input.server_id) {
        return Err("服务端不存在".into());
    }
    validate_proxy_name(&input.name)?;
    if input.local_ip.trim().is_empty() {
        return Err("本地 IP 不能为空".into());
    }
    if input.local_port == 0 {
        return Err("本地端口不合法".into());
    }
    match input.proxy_type {
        ProxyType::Tcp | ProxyType::Udp => {
            if !input.remote_port.is_some_and(|port| port > 0) {
                return Err("TCP/UDP 类型必须指定远端端口".into());
            }
        }
        ProxyType::Http | ProxyType::Https => {
            if input.custom_domains.is_empty()
                || input.custom_domains.iter().any(|d| d.trim().is_empty())
            {
                return Err("HTTP/HTTPS 类型必须指定 custom domains".into());
            }
        }
        ProxyType::Stcp => {}
    }
    Ok(())
}

#[tauri::command]
pub fn create_proxy(store: tauri::State<Store>, input: ProxyInput) -> Result<Proxy, String> {
    add_proxy(&store, input)
}

pub fn validate_new_proxy(input: &ProxyInput, data: &StoreData) -> Result<(), String> {
    validate_proxy_input(input, data)?;
    if data
        .proxies
        .iter()
        .any(|p| p.server_id == input.server_id && p.name == input.name)
    {
        return Err(format!(
            "同一 frps 服务端下已存在同名 proxy：{}",
            input.name
        ));
    }

    Ok(())
}

pub fn add_proxy(store: &Store, input: ProxyInput) -> Result<Proxy, String> {
    store.mutate(|d| {
        validate_new_proxy(&input, d)?;
        let proxy = Proxy {
            id: Uuid::new_v4().to_string(),
            server_id: input.server_id,
            name: input.name,
            description: input.description,
            proxy_type: input.proxy_type,
            local_ip: input.local_ip,
            local_port: input.local_port,
            remote_port: input.remote_port,
            custom_domains: input.custom_domains,
            enabled: false,
            created_at: Utc::now(),
        };
        d.proxies.push(proxy.clone());
        Ok(proxy)
    })
}

#[tauri::command]
pub fn update_proxy(
    store: tauri::State<Store>,
    id: String,
    input: ProxyInput,
) -> Result<Proxy, String> {
    let snap = store.snapshot();
    validate_proxy_input(&input, &snap)?;

    store.mutate(|d| {
        if d.proxies
            .iter()
            .any(|p| p.id != id && p.server_id == input.server_id && p.name == input.name)
        {
            return Err(format!(
                "同一 frps 服务端下已存在同名 proxy：{}",
                input.name
            ));
        }
        let p = d
            .proxies
            .iter_mut()
            .find(|p| p.id == id)
            .ok_or_else(|| "proxy 不存在".to_string())?;
        p.server_id = input.server_id.clone();
        p.name = input.name.clone();
        p.description = input.description.clone();
        p.proxy_type = input.proxy_type.clone();
        p.local_ip = input.local_ip.clone();
        p.local_port = input.local_port;
        p.remote_port = input.remote_port;
        p.custom_domains = input.custom_domains.clone();
        Ok(())
    })?;
    store
        .snapshot()
        .proxies
        .into_iter()
        .find(|p| p.id == id)
        .ok_or_else(|| "更新后找不到".into())
}

#[tauri::command]
pub fn delete_proxy(store: tauri::State<Store>, id: String) -> Result<(), String> {
    store.mutate(|d| {
        d.proxies.retain(|p| p.id != id);
        Ok(())
    })?;
    Ok(())
}

#[tauri::command]
pub fn set_proxy_enabled(
    store: tauri::State<Store>,
    id: String,
    enabled: bool,
) -> Result<Proxy, String> {
    store.mutate(|d| {
        let p = d
            .proxies
            .iter_mut()
            .find(|p| p.id == id)
            .ok_or_else(|| "proxy 不存在".to_string())?;
        p.enabled = enabled;
        Ok(())
    })?;
    store
        .snapshot()
        .proxies
        .into_iter()
        .find(|p| p.id == id)
        .ok_or_else(|| "更新后找不到".into())
}

// --- Settings ---

#[tauri::command]
pub fn update_settings(store: tauri::State<Store>, settings: Settings) -> Result<Settings, String> {
    store.mutate(|d| {
        d.settings = settings.clone();
        Ok(())
    })?;
    Ok(store.snapshot().settings)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn proxy_preview_and_create_share_validation_and_keep_it_disabled() {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::open(dir.path()).unwrap();
        let server = add_server(
            &store,
            ServerInput {
                name: "fixture".into(),
                host: "example.invalid".into(),
                port: 7000,
                token: String::new(),
                dashboard_url: None,
                dashboard_user: None,
                dashboard_pass: None,
            },
        )
        .unwrap();
        let input = |port| ProxyInput {
            server_id: server.id.clone(),
            name: "web".into(),
            description: String::new(),
            proxy_type: ProxyType::Tcp,
            local_ip: "127.0.0.1".into(),
            local_port: 8080,
            remote_port: Some(port),
            custom_domains: Vec::new(),
        };
        assert!(validate_new_proxy(&input(0), &store.snapshot()).is_err());
        validate_new_proxy(&input(18080), &store.snapshot()).unwrap();
        assert!(store.snapshot().proxies.is_empty());
        let proxy = add_proxy(&store, input(18080)).unwrap();
        assert!(!proxy.enabled);
        assert!(add_proxy(&store, input(18080)).is_err());
        let reloaded = Store::open(dir.path()).unwrap().snapshot();
        assert_eq!(reloaded.proxies.len(), 1);
        assert_eq!(reloaded.proxies[0].id, proxy.id);
        assert!(!reloaded.proxies[0].enabled);
    }

    #[test]
    fn a_failed_write_keeps_the_last_saved_state() {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::open(dir.path()).unwrap();
        fs::create_dir(&store.path).unwrap();
        assert!(store
            .mutate(|state| {
                state.settings.autostart = true;
                Ok(())
            })
            .is_err());
        assert!(!store.snapshot().settings.autostart);
    }
}
