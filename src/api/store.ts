import { invoke } from "@tauri-apps/api/core";
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

export const storeApi = {
  getState: () => invoke<StoreData>("get_state"),

  createProject: (input: ProjectInput) =>
    invoke<Project>("create_project", { input }),
  updateProject: (id: string, input: ProjectInput) =>
    invoke<Project>("update_project", { id, input }),
  deleteProject: (id: string) => invoke<void>("delete_project", { id }),

  createServer: (input: ServerInput) =>
    invoke<FrpsServer>("create_server", { input }),
  updateServer: (id: string, input: ServerInput) =>
    invoke<FrpsServer>("update_server", { id, input }),
  deleteServer: (id: string) => invoke<void>("delete_server", { id }),

  createProxy: (input: ProxyInput) =>
    invoke<Proxy>("create_proxy", { input }),
  updateProxy: (id: string, input: ProxyInput) =>
    invoke<Proxy>("update_proxy", { id, input }),
  deleteProxy: (id: string) => invoke<void>("delete_proxy", { id }),
  setProxyEnabled: (id: string, enabled: boolean) =>
    invoke<Proxy>("set_proxy_enabled", { id, enabled }),

  updateSettings: (settings: Settings) =>
    invoke<Settings>("update_settings", { settings }),
};
