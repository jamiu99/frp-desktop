import { fetch } from "@tauri-apps/plugin-http";
import type { FrpsServer } from "@/types/store";

/**
 * frps Admin API 类型（参考 https://github.com/fatedier/frp/blob/dev/doc/server_api.md）
 * 字段较多，只列我们用到的核心部分。
 */
export interface FrpsServerInfo {
  version: string;
  bind_port: number;
  vhost_http_port?: number;
  vhost_https_port?: number;
  total_traffic_in: number;
  total_traffic_out: number;
  cur_conns: number;
  client_counts: number;
  proxy_type_count?: Record<string, number>;
}

export interface FrpsProxyInfo {
  name: string;
  conf?: {
    name: string;
    type: string;
    local_ip?: string;
    local_port?: number;
    remote_port?: number;
    custom_domains?: string[];
  };
  today_traffic_in: number;
  today_traffic_out: number;
  cur_conns: number;
  last_start_time?: string;
  last_close_time?: string;
  status: string;
}

export interface FrpsProxiesResponse {
  proxies: FrpsProxyInfo[];
}

function authHeader(server: FrpsServer): Record<string, string> {
  if (!server.dashboard_user || !server.dashboard_pass) return {};
  const token = btoa(`${server.dashboard_user}:${server.dashboard_pass}`);
  return { Authorization: `Basic ${token}` };
}

async function call<T>(server: FrpsServer, path: string): Promise<T> {
  if (!server.dashboard_url) {
    throw new Error("该服务端未配置 dashboard URL");
  }
  const url = server.dashboard_url.replace(/\/+$/, "") + path;
  const res = await fetch(url, {
    method: "GET",
    headers: authHeader(server),
  });
  if (!res.ok) {
    throw new Error(`HTTP ${res.status} ${res.statusText} - ${url}`);
  }
  return (await res.json()) as T;
}

export const frpsApi = {
  serverInfo: (s: FrpsServer) => call<FrpsServerInfo>(s, "/api/serverinfo"),

  proxiesByType: async (s: FrpsServer, type: string) => {
    return call<FrpsProxiesResponse>(s, `/api/proxy/${type}`);
  },

  /** 一次拉所有类型的 proxy，合并返回 */
  async allProxies(s: FrpsServer) {
    const types = ["tcp", "udp", "http", "https", "stcp", "sudp", "xtcp"];
    const results = await Promise.allSettled(
      types.map((t) => call<FrpsProxiesResponse>(s, `/api/proxy/${t}`)),
    );
    const all: (FrpsProxyInfo & { type: string })[] = [];
    results.forEach((r, i) => {
      if (r.status === "fulfilled") {
        for (const p of r.value.proxies) all.push({ ...p, type: types[i] });
      }
    });
    return all;
  },
};
