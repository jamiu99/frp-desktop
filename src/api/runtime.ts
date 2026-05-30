import { invoke } from "@tauri-apps/api/core";
import type { ServerRuntime } from "@/types/runtime";

export const runtimeApi = {
  listRuntime: () => invoke<ServerRuntime[]>("list_runtime"),
  startProxy: (proxyId: string) =>
    invoke<void>("start_proxy", { proxyId }),
  stopProxy: (proxyId: string) =>
    invoke<void>("stop_proxy", { proxyId }),
  startServer: (serverId: string) =>
    invoke<void>("start_server", { serverId }),
  stopServer: (serverId: string) =>
    invoke<void>("stop_server", { serverId }),
  serverLogs: (serverId: string) =>
    invoke<string[]>("server_logs", { serverId }),
  checkFrpc: () => invoke<string>("check_frpc"),
};
