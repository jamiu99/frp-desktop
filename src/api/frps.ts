import { invoke } from "@tauri-apps/api/core";

export interface FrpsServerInfo {
  version: string;
  client_counts: number | null;
  cur_conns: number | null;
  total_traffic_in: number | null;
  total_traffic_out: number | null;
}

export interface FrpsProxyInfo {
  name: string;
  type: string;
  conf: {
    remote_port: number | null;
    custom_domains: string[] | null;
  } | null;
  today_traffic_in: number | null;
  today_traffic_out: number | null;
  cur_conns: number | null;
  status: string;
}

interface DashboardSnapshot {
  server_info: FrpsServerInfo | null;
  server_info_error: string | null;
  proxies: FrpsProxyInfo[];
  failed_types: Record<string, string>;
}

export const frpsApi = {
  overview: (serverId: string) =>
    invoke<DashboardSnapshot>("dashboard_overview", { serverId }),
};
