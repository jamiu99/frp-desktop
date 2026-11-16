import { defineStore } from "pinia";
import { ref } from "vue";
import { listen } from "@tauri-apps/api/event";
import { runtimeApi } from "@/api/runtime";
import type { ProxyRunStatus, ServerRuntime } from "@/types/runtime";
import type { Proxy } from "@/types/store";

type BadgeVariant = "success" | "warning" | "destructive" | "outline";

const MAX_LOG = 500;

interface ServerLogEvent {
  server_id: string;
  line: string;
}
interface ServerStatusEvent {
  server_id: string;
  status: ProxyRunStatus;
}

export const useRuntimeStore = defineStore("runtime", () => {
  /** key = server id */
  const runtimes = ref<Map<string, ServerRuntime>>(new Map());
  let listenersStarted = false;

  function ensure(id: string): ServerRuntime {
    let r = runtimes.value.get(id);
    if (!r) {
      r = {
        server_id: id,
        status: "stopped",
        pid: null,
        last_error: null,
        log_tail: [],
      };
      runtimes.value.set(id, r);
    }
    return r;
  }

  function statusOfServer(id: string): ProxyRunStatus {
    return runtimes.value.get(id)?.status ?? "stopped";
  }

  /**
   * proxy 的真实状态：enabled 只是「想要它跑」，是否真在跑看所属 server 的 frpc 进程。
   * 避免出现「显示已启用、实际没起 frpc」的错觉。
   */
  function proxyStatus(p: Proxy): { text: string; variant: BadgeVariant } {
    if (!p.enabled) return { text: "已停用", variant: "outline" };
    switch (statusOfServer(p.server_id)) {
      case "running":
        return { text: "运行中", variant: "success" };
      case "starting":
        return { text: "启动中", variant: "warning" };
      case "crashed":
        return { text: "已崩溃", variant: "destructive" };
      default:
        return { text: "未运行", variant: "warning" };
    }
  }

  async function refresh() {
    const list = await runtimeApi.listRuntime();
    runtimes.value = new Map(list.map((r) => [r.server_id, r]));
  }

  async function startListening() {
    if (listenersStarted) return;
    listenersStarted = true;
    await listen<ServerLogEvent>("server-log", (e) => {
      const r = ensure(e.payload.server_id);
      r.log_tail.push(e.payload.line);
      while (r.log_tail.length > MAX_LOG) r.log_tail.shift();
      runtimes.value = new Map(runtimes.value);
    });
    await listen<ServerStatusEvent>("server-status", (e) => {
      const r = ensure(e.payload.server_id);
      r.status = e.payload.status;
      if (e.payload.status === "stopped" || e.payload.status === "crashed") {
        r.pid = null;
      }
      runtimes.value = new Map(runtimes.value);
    });
  }

  async function startProxy(id: string) {
    await runtimeApi.startProxy(id);
    await refresh();
  }
  async function stopProxy(id: string) {
    await runtimeApi.stopProxy(id);
    await refresh();
  }
  async function startServer(id: string) {
    await runtimeApi.startServer(id);
    await refresh();
  }
  async function stopServer(id: string) {
    await runtimeApi.stopServer(id);
    await refresh();
  }

  return {
    runtimes,
    statusOfServer,
    proxyStatus,
    refresh,
    startListening,
    startProxy,
    stopProxy,
    startServer,
    stopServer,
  };
});
