import { invoke } from "@tauri-apps/api/core";
import type { ProxyRuntime } from "@/types/runtime";

export const runtimeApi = {
  listRuntime: () => invoke<ProxyRuntime[]>("list_runtime"),
  startProxy: (proxy_id: string) =>
    invoke<ProxyRuntime>("start_proxy", { proxyId: proxy_id }),
  stopProxy: (proxy_id: string) =>
    invoke<void>("stop_proxy", { proxyId: proxy_id }),
  proxyLogs: (proxy_id: string) =>
    invoke<string[]>("proxy_logs", { proxyId: proxy_id }),
  checkFrpc: () => invoke<string>("check_frpc"),
};
