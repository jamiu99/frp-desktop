import { defineStore } from "pinia";
import { computed, ref } from "vue";
import { storeApi } from "@/api/store";
import type {
  FrpsServer,
  Project,
  ProjectInput,
  Proxy,
  ProxyInput,
  ServerInput,
  Settings,
  StoreData,
} from "@/types/store";

/**
 * 应用全局数据 store。
 *
 * 设计：
 * - Rust 端是真理来源；前端只是镜像。
 * - 每次写操作完成后用 fetchAll() 重新拉一次（数据量小，简单可靠）
 * - 校验失败时 invoke 抛出 string 错误，由调用处 toast / 显示
 */
export const useDataStore = defineStore("data", () => {
  const projects = ref<Project[]>([]);
  const servers = ref<FrpsServer[]>([]);
  const proxies = ref<Proxy[]>([]);
  const settings = ref<Settings>({
    close_to_tray: false,
    autostart: false,
    frpc_path: null,
  });
  const loaded = ref(false);
  const loading = ref(false);

  function applyState(s: StoreData) {
    projects.value = s.projects;
    servers.value = s.servers;
    proxies.value = s.proxies;
    settings.value = s.settings;
    loaded.value = true;
  }

  async function fetchAll() {
    loading.value = true;
    try {
      applyState(await storeApi.getState());
    } finally {
      loading.value = false;
    }
  }

  // --- projects ---
  async function createProject(input: ProjectInput) {
    await storeApi.createProject(input);
    await fetchAll();
  }
  async function updateProject(id: string, input: ProjectInput) {
    await storeApi.updateProject(id, input);
    await fetchAll();
  }
  async function deleteProject(id: string) {
    await storeApi.deleteProject(id);
    await fetchAll();
  }

  // --- servers ---
  async function createServer(input: ServerInput) {
    await storeApi.createServer(input);
    await fetchAll();
  }
  async function updateServer(id: string, input: ServerInput) {
    await storeApi.updateServer(id, input);
    await fetchAll();
  }
  async function deleteServer(id: string) {
    await storeApi.deleteServer(id);
    await fetchAll();
  }

  // --- proxies ---
  async function createProxy(input: ProxyInput) {
    await storeApi.createProxy(input);
    await fetchAll();
  }
  async function updateProxy(id: string, input: ProxyInput) {
    await storeApi.updateProxy(id, input);
    await fetchAll();
  }
  async function deleteProxy(id: string) {
    await storeApi.deleteProxy(id);
    await fetchAll();
  }
  async function setProxyEnabled(id: string, enabled: boolean) {
    await storeApi.setProxyEnabled(id, enabled);
    await fetchAll();
  }

  // --- settings ---
  async function updateSettings(s: Settings) {
    await storeApi.updateSettings(s);
    await fetchAll();
  }

  // --- helpers ---
  const projectById = computed(() => {
    const m = new Map<string, Project>();
    for (const p of projects.value) m.set(p.id, p);
    return m;
  });
  const serverById = computed(() => {
    const m = new Map<string, FrpsServer>();
    for (const s of servers.value) m.set(s.id, s);
    return m;
  });

  return {
    projects,
    servers,
    proxies,
    settings,
    loaded,
    loading,
    fetchAll,
    createProject,
    updateProject,
    deleteProject,
    createServer,
    updateServer,
    deleteServer,
    createProxy,
    updateProxy,
    deleteProxy,
    setProxyEnabled,
    updateSettings,
    projectById,
    serverById,
  };
});
