import { defineStore } from "pinia";
import { ref } from "vue";
import { listen } from "@tauri-apps/api/event";
import { runtimeApi } from "@/api/runtime";
import type { ProxyRunStatus, ProxyRuntime } from "@/types/runtime";

const MAX_LOG = 500;

interface ProxyLogEvent {
  proxy_id: string;
  line: string;
}
interface ProxyStatusEvent {
  proxy_id: string;
  status: ProxyRunStatus;
}

export const useRuntimeStore = defineStore("runtime", () => {
  /** key = proxy id */
  const runtimes = ref<Map<string, ProxyRuntime>>(new Map());
  let listenersStarted = false;

  function ensure(id: string): ProxyRuntime {
    let r = runtimes.value.get(id);
    if (!r) {
      r = {
        proxy_id: id,
        status: "stopped",
        pid: null,
        last_error: null,
        log_tail: [],
      };
      runtimes.value.set(id, r);
    }
    return r;
  }

  function set(r: ProxyRuntime) {
    runtimes.value.set(r.proxy_id, r);
    runtimes.value = new Map(runtimes.value); // 触发响应式
  }

  function statusOf(id: string): ProxyRunStatus {
    return runtimes.value.get(id)?.status ?? "stopped";
  }

  async function refresh() {
    const list = await runtimeApi.listRuntime();
    runtimes.value = new Map(list.map((r) => [r.proxy_id, r]));
  }

  async function startListening() {
    if (listenersStarted) return;
    listenersStarted = true;
    await listen<ProxyLogEvent>("proxy-log", (e) => {
      const r = ensure(e.payload.proxy_id);
      r.log_tail.push(e.payload.line);
      while (r.log_tail.length > MAX_LOG) r.log_tail.shift();
      runtimes.value = new Map(runtimes.value);
    });
    await listen<ProxyStatusEvent>("proxy-status", (e) => {
      const r = ensure(e.payload.proxy_id);
      r.status = e.payload.status;
      runtimes.value = new Map(runtimes.value);
    });
  }

  async function start(id: string) {
    const r = await runtimeApi.startProxy(id);
    set(r);
  }
  async function stop(id: string) {
    await runtimeApi.stopProxy(id);
    const r = runtimes.value.get(id);
    if (r) {
      r.status = "stopped";
      r.pid = null;
      runtimes.value = new Map(runtimes.value);
    }
  }

  return { runtimes, statusOf, refresh, startListening, start, stop };
});
