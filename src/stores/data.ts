import { defineStore } from "pinia";
import { computed, ref } from "vue";
import { storeApi } from "@/api/store";
import type {
  FrpsServer,
  Proxy,
  ProxyInput,
  ServerInput,
  Settings,
  StoreData,
} from "@/types/store";

export const useDataStore = defineStore("data", () => {
  const servers = ref<FrpsServer[]>([]);
  const proxies = ref<Proxy[]>([]);
  const settings = ref<Settings>({
    close_to_tray: false,
    autostart: false,
    frpc_path: null,
    show_frpc_console: false,
  });
  const loaded = ref(false);
  const loading = ref(false);

  function applyState(s: StoreData) {
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

  async function updateSettings(s: Settings) {
    await storeApi.updateSettings(s);
    await fetchAll();
  }

  const serverById = computed(() => {
    const m = new Map<string, FrpsServer>();
    for (const s of servers.value) m.set(s.id, s);
    return m;
  });

  const proxiesByServer = computed(() => {
    const m = new Map<string, Proxy[]>();
    for (const p of proxies.value) {
      const arr = m.get(p.server_id) ?? [];
      arr.push(p);
      m.set(p.server_id, arr);
    }
    return m;
  });

  return {
    servers,
    proxies,
    settings,
    loaded,
    loading,
    fetchAll,
    createServer,
    updateServer,
    deleteServer,
    createProxy,
    updateProxy,
    deleteProxy,
    setProxyEnabled,
    updateSettings,
    serverById,
    proxiesByServer,
  };
});
