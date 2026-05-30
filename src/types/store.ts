// 与 src-tauri/src/store.rs 同步

export interface FrpsServer {
  id: string;
  name: string;
  host: string;
  port: number;
  token: string;
  dashboard_url: string | null;
  dashboard_user: string | null;
  dashboard_pass: string | null;
  created_at: string;
}

export type ProxyType = "tcp" | "udp" | "http" | "https" | "stcp";

export interface Proxy {
  id: string;
  server_id: string;
  name: string;
  description: string;
  proxy_type: ProxyType;
  local_ip: string;
  local_port: number;
  remote_port: number | null;
  custom_domains: string[];
  enabled: boolean;
  created_at: string;
}

export interface Settings {
  close_to_tray: boolean;
  autostart: boolean;
  frpc_path: string | null;
  show_frpc_console: boolean;
}

export interface StoreData {
  servers: FrpsServer[];
  proxies: Proxy[];
  settings: Settings;
}

export interface ServerInput {
  name: string;
  host: string;
  port: number;
  token: string;
  dashboard_url?: string | null;
  dashboard_user?: string | null;
  dashboard_pass?: string | null;
}

export interface ProxyInput {
  server_id: string;
  name: string;
  description?: string;
  proxy_type: ProxyType;
  local_ip: string;
  local_port: number;
  remote_port?: number | null;
  custom_domains?: string[];
}
