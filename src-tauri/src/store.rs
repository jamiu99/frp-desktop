//! 数据存储：单 JSON 文件，存在 app_data_dir/store.json
//!
//! 设计取舍：
//! - 桌面单用户应用，数据量小（项目<10、服务端<10、proxy<100），SQLite 是过度设计
//! - 单文件读全量、改全量、写全量；写入用 tmp + rename 保证原子性
//! - token / dashboard 密码明文存（文档已说明），文件权限默认靠 OS 用户目录隔离
//!
//! 模型见 docs/STATUS.md 的 ADR。

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;
use tauri::{AppHandle, Manager};
use uuid::Uuid;

// ---------- 数据模型 ----------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Project {
    pub id: String,
    pub name: String,
    pub description: String,
    pub color: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FrpsServer {
    pub id: String,
    pub name: String,
    /// frpc 连接到 frps 用的地址 (host:port)
    pub host: String,
    pub port: u16,
    /// frp token (明文)
    pub token: String,
    /// frps dashboard URL（含协议），如 http://frps.example.com:5002
    pub dashboard_url: Option<String>,
    pub dashboard_user: Option<String>,
    pub dashboard_pass: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
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
    pub project_id: String,
    pub server_id: String,
    /// 拼出来的 frpc proxy name，规则 {project_slug}-{purpose}
    pub name: String,
    /// 用户填的"用途"，如 ssh / webui
    pub purpose: String,
    /// 必填，>= 10 字符
    pub description: String,
    pub proxy_type: ProxyType,
    pub local_ip: String,
    pub local_port: u16,
    pub remote_port: Option<u16>,
    pub custom_domains: Vec<String>,
    pub enabled: bool,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Settings {
    /// 关闭主窗口时是否最小化到托盘
    pub close_to_tray: bool,
    /// 开机自启
    pub autostart: bool,
    /// 自定义 frpc 二进制路径（None 表示用打包内置的）
    pub frpc_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct StoreData {
    pub projects: Vec<Project>,
    pub servers: Vec<FrpsServer>,
    pub proxies: Vec<Proxy>,
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

        let data = if path.exists() {
            let txt = fs::read_to_string(&path)
                .map_err(|e| format!("read store: {e}"))?;
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
        // 原子写：先写 .tmp 再 rename
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

const PURPOSE_BLACKLIST: &[&str] = &[
    "test", "test1", "test2", "temp", "tmp", "a", "b", "c", "1", "2",
    "xxx", "yyy", "demo", "foo", "bar",
];

const NAME_RE: &str = r"^[a-z0-9]([a-z0-9-]{0,30}[a-z0-9])?$";

fn is_valid_slug(s: &str) -> bool {
    let re = regex_lite::Regex::new(NAME_RE).unwrap();
    re.is_match(s)
}

fn validate_purpose(p: &str) -> Result<(), String> {
    if p.is_empty() {
        return Err("用途（purpose）必填".into());
    }
    if !is_valid_slug(p) {
        return Err(
            "用途只能小写字母/数字/连字符，2~32 字符，例：ssh、webui、api-gateway"
                .into(),
        );
    }
    if PURPOSE_BLACKLIST.contains(&p) {
        return Err(format!("不允许使用占位词作为用途：{p}"));
    }
    Ok(())
}

fn validate_project_name(name: &str) -> Result<(), String> {
    if name.trim().is_empty() {
        return Err("项目名必填".into());
    }
    if !is_valid_slug(name) {
        return Err(
            "项目名只能小写字母/数字/连字符，2~32 字符，例：nas、company-vpn"
                .into(),
        );
    }
    if PURPOSE_BLACKLIST.contains(&name) {
        return Err(format!("不允许使用占位词作为项目名：{name}"));
    }
    Ok(())
}

fn validate_description(d: &str) -> Result<(), String> {
    let chars = d.chars().count();
    if chars < 10 {
        return Err(format!("描述至少 10 个字符（当前 {}）", chars));
    }
    Ok(())
}

// ---------- Tauri Commands ----------

#[tauri::command]
pub fn get_state(store: tauri::State<Store>) -> StoreData {
    store.snapshot()
}

// --- Project ---

#[derive(Debug, Deserialize)]
pub struct ProjectInput {
    pub name: String,
    pub description: String,
    pub color: Option<String>,
}

#[tauri::command]
pub fn create_project(
    store: tauri::State<Store>,
    input: ProjectInput,
) -> Result<Project, String> {
    validate_project_name(&input.name)?;
    validate_description(&input.description)?;

    let snap = store.snapshot();
    if snap.projects.iter().any(|p| p.name == input.name) {
        return Err(format!("项目名已存在：{}", input.name));
    }

    let project = Project {
        id: Uuid::new_v4().to_string(),
        name: input.name,
        description: input.description,
        color: input.color,
        created_at: Utc::now(),
    };

    let result = project.clone();
    store.mutate(|d| {
        d.projects.push(project);
        Ok(())
    })?;
    Ok(result)
}

#[tauri::command]
pub fn update_project(
    store: tauri::State<Store>,
    id: String,
    input: ProjectInput,
) -> Result<Project, String> {
    validate_project_name(&input.name)?;
    validate_description(&input.description)?;

    store.mutate(|d| {
        // 重名检查（排除自己）
        if d.projects.iter().any(|p| p.id != id && p.name == input.name) {
            return Err(format!("项目名已存在：{}", input.name));
        }
        let p = d
            .projects
            .iter_mut()
            .find(|p| p.id == id)
            .ok_or_else(|| format!("项目不存在：{id}"))?;
        p.name = input.name.clone();
        p.description = input.description.clone();
        p.color = input.color.clone();
        Ok(())
    })?;

    let snap = store.snapshot();
    snap.projects
        .into_iter()
        .find(|p| p.id == id)
        .ok_or_else(|| "更新后找不到".into())
}

#[tauri::command]
pub fn delete_project(
    store: tauri::State<Store>,
    id: String,
) -> Result<(), String> {
    store.mutate(|d| {
        if d.proxies.iter().any(|p| p.project_id == id) {
            return Err("项目下还有 proxy，无法删除（请先删除或迁移其下的 proxy）".into());
        }
        d.projects.retain(|p| p.id != id);
        Ok(())
    })?;
    Ok(())
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
        return Err("服务端名称必填".into());
    }
    if s.host.trim().is_empty() {
        return Err("frps host 必填".into());
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
        if d.proxies.iter().any(|p| p.server_id == id) {
            return Err("服务端下还有 proxy，无法删除".into());
        }
        d.servers.retain(|s| s.id != id);
        Ok(())
    })?;
    Ok(())
}

// --- Proxy ---

#[derive(Debug, Deserialize)]
pub struct ProxyInput {
    pub project_id: String,
    pub server_id: String,
    pub purpose: String,
    pub description: String,
    pub proxy_type: ProxyType,
    pub local_ip: String,
    pub local_port: u16,
    pub remote_port: Option<u16>,
    pub custom_domains: Option<Vec<String>>,
    pub enabled: Option<bool>,
}

fn validate_proxy_input(input: &ProxyInput, data: &StoreData) -> Result<String, String> {
    let project = data
        .projects
        .iter()
        .find(|p| p.id == input.project_id)
        .ok_or_else(|| "项目不存在".to_string())?;
    if !data.servers.iter().any(|s| s.id == input.server_id) {
        return Err("服务端不存在".into());
    }

    validate_purpose(&input.purpose)?;
    validate_description(&input.description)?;

    if input.local_ip.trim().is_empty() {
        return Err("本地 IP 必填".into());
    }
    if input.local_port == 0 {
        return Err("本地端口不合法".into());
    }

    match input.proxy_type {
        ProxyType::Tcp | ProxyType::Udp => {
            if input.remote_port.is_none() {
                return Err("TCP/UDP 类型必须指定 remote_port".into());
            }
        }
        ProxyType::Http | ProxyType::Https => {
            if input.custom_domains.as_ref().is_none_or(|v| v.is_empty()) {
                return Err("HTTP/HTTPS 类型必须指定 custom_domains".into());
            }
        }
        ProxyType::Stcp => {}
    }

    Ok(format!("{}-{}", project.name, input.purpose))
}

#[tauri::command]
pub fn create_proxy(
    store: tauri::State<Store>,
    input: ProxyInput,
) -> Result<Proxy, String> {
    let snap = store.snapshot();
    let name = validate_proxy_input(&input, &snap)?;

    // 同 server 下查重
    if snap
        .proxies
        .iter()
        .any(|p| p.server_id == input.server_id && p.name == name)
    {
        return Err(format!("同一 frps 服务端下已存在 proxy：{name}"));
    }

    let proxy = Proxy {
        id: Uuid::new_v4().to_string(),
        project_id: input.project_id,
        server_id: input.server_id,
        name,
        purpose: input.purpose,
        description: input.description,
        proxy_type: input.proxy_type,
        local_ip: input.local_ip,
        local_port: input.local_port,
        remote_port: input.remote_port,
        custom_domains: input.custom_domains.unwrap_or_default(),
        enabled: input.enabled.unwrap_or(false),
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
    let name = validate_proxy_input(&input, &snap)?;

    store.mutate(|d| {
        if d.proxies
            .iter()
            .any(|p| p.id != id && p.server_id == input.server_id && p.name == name)
        {
            return Err(format!("同一 frps 服务端下已存在 proxy：{name}"));
        }
        let p = d
            .proxies
            .iter_mut()
            .find(|p| p.id == id)
            .ok_or_else(|| "proxy 不存在".to_string())?;
        p.project_id = input.project_id.clone();
        p.server_id = input.server_id.clone();
        p.name = name.clone();
        p.purpose = input.purpose.clone();
        p.description = input.description.clone();
        p.proxy_type = input.proxy_type.clone();
        p.local_ip = input.local_ip.clone();
        p.local_port = input.local_port;
        p.remote_port = input.remote_port;
        p.custom_domains = input.custom_domains.clone().unwrap_or_default();
        if let Some(en) = input.enabled {
            p.enabled = en;
        }
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
