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
use std::path::PathBuf;
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
        fs::create_dir_all(&dir).map_err(|e| format!("create dir: {e}"))?;
        let path = dir.join("store.json");

        let data: StoreData = if path.exists() {
            let txt = fs::read_to_string(&path)
                .map_err(|e| format!("read store: {e}"))?;
            // 旧文件中的 proxy 可能含 project_id 字段——serde 默认忽略未知字段
            serde_json::from_str(&txt)
                .map_err(|e| format!("parse store: {e}"))?
        } else {
            StoreData::default()
        };

        Ok(Self {
            path,
            data: Mutex::new(data),
        })
    }

    fn persist(&self, data: &StoreData) -> Result<(), String> {
        let txt = serde_json::to_string_pretty(data)
            .map_err(|e| format!("serialize: {e}"))?;
        let tmp = self.path.with_extension("json.tmp");
        fs::write(&tmp, txt).map_err(|e| format!("write tmp: {e}"))?;
        fs::rename(&tmp, &self.path).map_err(|e| format!("rename: {e}"))?;
        Ok(())
    }

    pub fn snapshot(&self) -> StoreData {
        self.data.lock().unwrap().clone()
    }

    pub fn mutate<F>(&self, f: F) -> Result<StoreData, String>
    where
        F: FnOnce(&mut StoreData) -> Result<(), String>,
    {
        let mut guard = self.data.lock().unwrap();
        f(&mut guard)?;
        self.persist(&guard)?;
        Ok(guard.clone())
    }
}

// ---------- 校验 ----------

const NAME_RE: &str = r"^[A-Za-z0-9_][A-Za-z0-9_.-]{0,63}$";

fn is_valid_name(s: &str) -> bool {
    regex_lite::Regex::new(NAME_RE).unwrap().is_match(s)
}

fn validate_proxy_name(name: &str) -> Result<(), String> {
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

#[derive(Debug, Deserialize)]
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
pub fn create_server(
    store: tauri::State<Store>,
    input: ServerInput,
) -> Result<FrpsServer, String> {
    validate_server_input(&input)?;
    let snap = store.snapshot();
    if snap.servers.iter().any(|s| s.name == input.name) {
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
    let result = server.clone();
    store.mutate(|d| {
        d.servers.push(server);
        Ok(())
    })?;
    Ok(result)
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
pub fn delete_server(
    store: tauri::State<Store>,
    id: String,
) -> Result<(), String> {
    store.mutate(|d| {
        d.proxies.retain(|p| p.server_id != id);
        d.servers.retain(|s| s.id != id);
        Ok(())
    })?;
    Ok(())
}

// --- Proxy ---

#[derive(Debug, Deserialize)]
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
            if input.remote_port.is_none() {
                return Err("TCP/UDP 类型必须指定远端端口".into());
            }
        }
        ProxyType::Http | ProxyType::Https => {
            if input.custom_domains.is_empty() {
                return Err("HTTP/HTTPS 类型必须指定 custom domains".into());
            }
        }
        ProxyType::Stcp => {}
    }
    Ok(())
}

#[tauri::command]
pub fn create_proxy(
    store: tauri::State<Store>,
    input: ProxyInput,
) -> Result<Proxy, String> {
    let snap = store.snapshot();
    validate_proxy_input(&input, &snap)?;

    if snap
        .proxies
        .iter()
        .any(|p| p.server_id == input.server_id && p.name == input.name)
    {
        return Err(format!("同一 frps 服务端下已存在同名 proxy：{}", input.name));
    }

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
    let result = proxy.clone();
    store.mutate(|d| {
        d.proxies.push(proxy);
        Ok(())
    })?;
    Ok(result)
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
            return Err(format!("同一 frps 服务端下已存在同名 proxy：{}", input.name));
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
pub fn delete_proxy(
    store: tauri::State<Store>,
    id: String,
) -> Result<(), String> {
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
pub fn update_settings(
    store: tauri::State<Store>,
    settings: Settings,
) -> Result<Settings, String> {
    store.mutate(|d| {
        d.settings = settings.clone();
        Ok(())
    })?;
    Ok(store.snapshot().settings)
}
