//! 本机网络端口查看
//!
//! 用 netstat2 拿 socket 列表 + sysinfo 拿进程名（按 PID 查）。
//! 跨平台（Windows / macOS / Linux）。

use netstat2::{
    AddressFamilyFlags, ProtocolFlags, ProtocolSocketInfo, SocketInfo,
    get_sockets_info,
};
use serde::Serialize;
use std::collections::HashMap;
use sysinfo::{Pid, ProcessRefreshKind, RefreshKind, System};

#[derive(Debug, Clone, Serialize)]
pub struct PortEntry {
    /// 进程名（取不到时为 "unknown"）
    pub process: String,
    /// 进程 PID（0 表示未关联到具体进程）
    pub pid: u32,
    /// "tcp" / "tcp6" / "udp" / "udp6"
    pub protocol: String,
    /// 本地 IP（如 127.0.0.1 / 0.0.0.0 / ::）
    pub local_address: String,
    /// 本地端口
    pub local_port: u16,
    /// TCP 状态（UDP 为 None）
    pub state: Option<String>,
}

#[tauri::command]
pub fn list_ports() -> Result<Vec<PortEntry>, String> {
    let af_flags = AddressFamilyFlags::IPV4 | AddressFamilyFlags::IPV6;
    let proto_flags = ProtocolFlags::TCP | ProtocolFlags::UDP;

    let sockets = get_sockets_info(af_flags, proto_flags)
        .map_err(|e| format!("read sockets failed: {e}"))?;

    let pids: Vec<u32> = sockets.iter().flat_map(|s| s.associated_pids.clone()).collect();
    let pid_to_name = collect_process_names(&pids);

    let mut entries: Vec<PortEntry> = Vec::with_capacity(sockets.len());
    for s in sockets {
        entries.extend(socket_to_entries(&s, &pid_to_name));
    }
    Ok(entries)
}

fn collect_process_names(pids: &[u32]) -> HashMap<u32, String> {
    let mut map = HashMap::new();
    if pids.is_empty() {
        return map;
    }

    let sys = System::new_with_specifics(
        RefreshKind::new().with_processes(ProcessRefreshKind::new()),
    );
    for &pid in pids {
        if let Some(p) = sys.process(Pid::from_u32(pid)) {
            map.insert(pid, p.name().to_string_lossy().to_string());
        }
    }
    map
}

fn socket_to_entries(
    s: &SocketInfo,
    pid_to_name: &HashMap<u32, String>,
) -> Vec<PortEntry> {
    let (protocol, local_address, local_port, state) = match &s.protocol_socket_info {
        ProtocolSocketInfo::Tcp(tcp) => {
            let proto = if tcp.local_addr.is_ipv6() { "tcp6" } else { "tcp" };
            (
                proto.to_string(),
                tcp.local_addr.to_string(),
                tcp.local_port,
                Some(format!("{:?}", tcp.state)),
            )
        }
        ProtocolSocketInfo::Udp(udp) => {
            let proto = if udp.local_addr.is_ipv6() { "udp6" } else { "udp" };
            (
                proto.to_string(),
                udp.local_addr.to_string(),
                udp.local_port,
                None,
            )
        }
    };

    if s.associated_pids.is_empty() {
        return vec![PortEntry {
            process: "unknown".into(),
            pid: 0,
            protocol,
            local_address,
            local_port,
            state,
        }];
    }

    s.associated_pids
        .iter()
        .map(|&pid| PortEntry {
            process: pid_to_name
                .get(&pid)
                .cloned()
                .unwrap_or_else(|| "unknown".into()),
            pid,
            protocol: protocol.clone(),
            local_address: local_address.clone(),
            local_port,
            state: state.clone(),
        })
        .collect()
}
