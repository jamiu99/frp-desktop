//! 配置导入 / 导出
//!
//! - 导出：所有 frps 服务端 + proxy 写成一个 JSON（含 token / dashboard 密码明文）。
//!   本机设置（frpc 路径、开机自启等）不导出——换台机器就不对了。
//! - 导入：两种模式
//!   - Merge：服务端按 name 匹配、proxy 按 (服务端, name) 匹配，存在则更新，不存在则新增
//!   - Replace：清空现有服务端和 proxy，整体替换
//!   新导入的 proxy 一律 enabled=false：避免和导出那台机器抢同一个 remotePort，由用户手动启动。
//! - dry_run=true 只返回统计、不落盘，前端用来做导入前预览。
//! - 兼容直接导入应用自己的 store.json（同样有 servers / proxies 字段）。

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fs;
use tauri::AppHandle;
use uuid::Uuid;

use crate::frpc::{self, Runtime};
use crate::store::{validate_proxy_name, FrpsServer, Proxy, Store, StoreData};

const FORMAT: &str = "frp-desktop-config";
const FORMAT_VERSION: u32 = 1;

#[derive(Debug, Serialize, Deserialize)]
struct ConfigBundle {
    #[serde(default)]
    format: String,
    #[serde(default)]
    version: u32,
    #[serde(default)]
    app_version: String,
    #[serde(default)]
    exported_at: Option<DateTime<Utc>>,
    #[serde(default)]
    servers: Vec<FrpsServer>,
    #[serde(default)]
    proxies: Vec<Proxy>,
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ImportMode {
    Merge,
    Replace,
}

#[derive(Debug, Default, Serialize)]
pub struct ImportReport {
    pub servers_added: usize,
    pub servers_updated: usize,
    pub servers_removed: usize,
    pub proxies_added: usize,
    pub proxies_updated: usize,
    pub proxies_removed: usize,
}

#[derive(Debug, Serialize)]
pub struct ExportReport {
    pub servers: usize,
    pub proxies: usize,
}

fn read_bundle(path: &str) -> Result<ConfigBundle, String> {
    let txt = fs::read_to_string(path).map_err(|e| format!("读取文件失败: {e}"))?;
    let b: ConfigBundle =
        serde_json::from_str(&txt).map_err(|e| format!("不是有效的配置文件: {e}"))?;
    if !b.format.is_empty() && b.format != FORMAT {
        return Err(format!("不认识的文件格式：{}", b.format));
    }
    if b.version > FORMAT_VERSION {
        return Err(format!(
            "配置文件版本 v{} 比当前应用支持的 v{FORMAT_VERSION} 新，请先升级应用",
            b.version
        ));
    }
    if b.servers.is_empty() && b.proxies.is_empty() {
        return Err("文件里没有任何服务端或 proxy".into());
    }
    validate_bundle(&b)?;
    Ok(b)
}

fn validate_bundle(b: &ConfigBundle) -> Result<(), String> {
    let mut names = HashSet::new();
    let mut ids = HashSet::new();
    for s in &b.servers {
        if s.name.trim().is_empty() || s.host.trim().is_empty() || s.port == 0 {
            return Err(format!("服务端「{}」缺少名称 / host / port", s.name));
        }
        if !names.insert(s.name.as_str()) {
            return Err(format!("文件中服务端名称重复：{}", s.name));
        }
        ids.insert(s.id.as_str());
    }
    let mut keys = HashSet::new();
    for p in &b.proxies {
        if !ids.contains(p.server_id.as_str()) {
            return Err(format!("proxy「{}」引用的服务端不在文件中", p.name));
        }
        validate_proxy_name(&p.name).map_err(|e| format!("proxy「{}」: {e}", p.name))?;
        if !keys.insert((p.server_id.as_str(), p.name.as_str())) {
            return Err(format!("文件中同一服务端下 proxy 重复：{}", p.name));
        }
    }
    Ok(())
}

/// 把 bundle 应用到 d 上，返回统计和「被改动的已有服务端 id」（用于重启其 frpc）。
fn apply(d: &mut StoreData, b: &ConfigBundle, mode: ImportMode) -> (ImportReport, HashSet<String>) {
    let mut r = ImportReport::default();
    let mut touched = HashSet::new();

    if let ImportMode::Replace = mode {
        r.servers_removed = d.servers.len();
        r.proxies_removed = d.proxies.len();
        d.servers.clear();
        d.proxies.clear();
    }

    // 文件里的 server id → 本地 server id
    let mut id_map: HashMap<&str, String> = HashMap::new();
    for fs in &b.servers {
        if let Some(ex) = d.servers.iter_mut().find(|s| s.name == fs.name) {
            ex.host = fs.host.clone();
            ex.port = fs.port;
            ex.token = fs.token.clone();
            ex.dashboard_url = fs.dashboard_url.clone();
            ex.dashboard_user = fs.dashboard_user.clone();
            ex.dashboard_pass = fs.dashboard_pass.clone();
            id_map.insert(fs.id.as_str(), ex.id.clone());
            touched.insert(ex.id.clone());
            r.servers_updated += 1;
        } else {
            let mut s = fs.clone();
            s.id = Uuid::new_v4().to_string();
            id_map.insert(fs.id.as_str(), s.id.clone());
            d.servers.push(s);
            r.servers_added += 1;
        }
    }

    for fp in &b.proxies {
        let sid = &id_map[fp.server_id.as_str()];
        if let Some(ex) = d
            .proxies
            .iter_mut()
            .find(|p| &p.server_id == sid && p.name == fp.name)
        {
            // 保留本地 id / enabled / created_at
            ex.description = fp.description.clone();
            ex.proxy_type = fp.proxy_type.clone();
            ex.local_ip = fp.local_ip.clone();
            ex.local_port = fp.local_port;
            ex.remote_port = fp.remote_port;
            ex.custom_domains = fp.custom_domains.clone();
            r.proxies_updated += 1;
        } else {
            let mut p = fp.clone();
            p.id = Uuid::new_v4().to_string();
            p.server_id = sid.clone();
            p.enabled = false;
            d.proxies.push(p);
            r.proxies_added += 1;
        }
    }

    (r, touched)
}

#[tauri::command]
pub fn export_config(
    app: AppHandle,
    store: tauri::State<Store>,
    path: String,
) -> Result<ExportReport, String> {
    let snap = store.snapshot();
    let bundle = ConfigBundle {
        format: FORMAT.into(),
        version: FORMAT_VERSION,
        app_version: app.package_info().version.to_string(),
        exported_at: Some(Utc::now()),
        servers: snap.servers,
        proxies: snap.proxies,
    };
    let txt = serde_json::to_string_pretty(&bundle).map_err(|e| format!("序列化失败: {e}"))?;
    fs::write(&path, txt).map_err(|e| format!("写入文件失败: {e}"))?;
    Ok(ExportReport {
        servers: bundle.servers.len(),
        proxies: bundle.proxies.len(),
    })
}

#[tauri::command]
pub fn import_config(
    app: AppHandle,
    store: tauri::State<Store>,
    runtime: tauri::State<Runtime>,
    path: String,
    mode: ImportMode,
    dry_run: bool,
) -> Result<ImportReport, String> {
    let bundle = read_bundle(&path)?;

    if dry_run {
        let mut d = store.snapshot();
        return Ok(apply(&mut d, &bundle, mode).0);
    }

    let mut out = None;
    store.mutate(|d| {
        out = Some(apply(d, &bundle, mode));
        Ok(())
    })?;
    let (report, touched) = out.expect("mutate 成功时 apply 一定执行过");

    // 让正在跑的 frpc 跟上新配置：服务端被删 → 停；配置被更新 → 用新配置重启
    frpc::reconcile_after_import(&app, &runtime, &store, &touched);
    Ok(report)
}
