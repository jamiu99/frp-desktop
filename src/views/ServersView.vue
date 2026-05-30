<script setup lang="ts">
import { ref } from "vue";
import { useRouter } from "vue-router";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import {
  Plus,
  Server,
  Pencil,
  Trash2,
  ExternalLink,
  Activity,
  Play,
  Square,
  FileText,
} from "@lucide/vue";
import ServerFormDialog from "@/components/forms/ServerFormDialog.vue";
import ProxyFormDialog from "@/components/forms/ProxyFormDialog.vue";
import ProxyLogsDialog from "@/components/forms/ProxyLogsDialog.vue";
import { ConfirmDialog } from "@/components/ui/confirm";
import { useDataStore } from "@/stores/data";
import { useRuntimeStore } from "@/stores/runtime";
import { toast } from "@/components/ui/toast";
import { STATUS_LABEL } from "@/types/runtime";
import type { FrpsServer, Proxy } from "@/types/store";

const data = useDataStore();
const runtime = useRuntimeStore();
const router = useRouter();

// dialogs
const serverFormOpen = ref(false);
const editingServer = ref<FrpsServer | null>(null);
const proxyFormOpen = ref(false);
const editingProxy = ref<Proxy | null>(null);
const proxyDefaultServerId = ref<string | undefined>(undefined);
const logsOpen = ref(false);
const logsServer = ref<FrpsServer | null>(null);

// confirm
const confirmOpen = ref(false);
const pendingDelete = ref<
  | { kind: "server"; server: FrpsServer }
  | { kind: "proxy"; proxy: Proxy }
  | null
>(null);
const deleting = ref(false);

function openCreateServer() {
  editingServer.value = null;
  serverFormOpen.value = true;
}
function openEditServer(s: FrpsServer) {
  editingServer.value = s;
  serverFormOpen.value = true;
}
function askDeleteServer(s: FrpsServer) {
  pendingDelete.value = { kind: "server", server: s };
  confirmOpen.value = true;
}

function openCreateProxy(s: FrpsServer) {
  editingProxy.value = null;
  proxyDefaultServerId.value = s.id;
  proxyFormOpen.value = true;
}
function openEditProxy(p: Proxy) {
  editingProxy.value = p;
  proxyFormOpen.value = true;
}
function askDeleteProxy(p: Proxy) {
  pendingDelete.value = { kind: "proxy", proxy: p };
  confirmOpen.value = true;
}

async function doDelete() {
  if (!pendingDelete.value) return;
  deleting.value = true;
  try {
    if (pendingDelete.value.kind === "server") {
      await data.deleteServer(pendingDelete.value.server.id);
    } else {
      await data.deleteProxy(pendingDelete.value.proxy.id);
    }
    toast.success("已删除");
    confirmOpen.value = false;
  } catch (e) {
    toast.fromError(e);
  } finally {
    deleting.value = false;
  }
}

async function toggleProxyRun(p: Proxy) {
  try {
    if (p.enabled) {
      await runtime.stopProxy(p.id);
      await data.fetchAll();
      toast.success(`已停止 ${p.name}`);
    } else {
      await runtime.startProxy(p.id);
      await data.fetchAll();
      toast.success(`已启动 ${p.name}`);
    }
  } catch (e) {
    toast.fromError(e);
  }
}

function gotoDashboard(s: FrpsServer) {
  router.push({ name: "dashboard", params: { serverId: s.id } });
}
function openLogs(s: FrpsServer) {
  logsServer.value = s;
  logsOpen.value = true;
}

function statusBadge(serverId: string) {
  const s = runtime.statusOfServer(serverId);
  return {
    text: STATUS_LABEL[s],
    variant:
      s === "running"
        ? ("success" as const)
        : s === "starting"
          ? ("warning" as const)
          : s === "crashed"
            ? ("destructive" as const)
            : ("outline" as const),
  };
}

function proxyTypeStyle(_t: string) {
  return "secondary" as const;
}
</script>

<template>
  <div class="space-y-6">
    <div class="flex items-center justify-between">
      <div>
        <h2 class="text-2xl font-semibold tracking-tight">frps 服务端</h2>
        <p class="text-sm text-muted-foreground">
          每个服务端下的 proxy 共用同一个 frpc 进程，启停互不影响其他服务端
        </p>
      </div>
      <Button @click="openCreateServer">
        <Plus class="h-4 w-4" />
        添加服务端
      </Button>
    </div>

    <Card v-if="data.servers.length === 0">
      <CardHeader class="flex flex-col items-center text-center">
        <div
          class="mx-auto mb-2 flex h-12 w-12 items-center justify-center rounded-full bg-secondary"
        >
          <Server class="h-6 w-6 text-muted-foreground" />
        </div>
        <CardTitle>还没有服务端</CardTitle>
        <CardDescription>添加一个 frps 服务端，例如 frps.example.com:7000</CardDescription>
      </CardHeader>
      <CardContent class="flex justify-center pb-6">
        <Button variant="outline" @click="openCreateServer">
          <Plus class="h-4 w-4" />
          添加第一个
        </Button>
      </CardContent>
    </Card>

    <div v-else class="space-y-4">
      <Card v-for="s in data.servers" :key="s.id">
        <CardHeader class="border-b">
          <div class="flex items-start justify-between gap-2">
            <div class="min-w-0">
              <CardTitle class="flex items-center gap-2">
                <Server class="h-4 w-4 text-primary shrink-0" />
                <span class="truncate">{{ s.name }}</span>
                <Badge :variant="statusBadge(s.id).variant">
                  {{ statusBadge(s.id).text }}
                </Badge>
              </CardTitle>
              <CardDescription class="mt-1 font-mono text-xs">
                {{ s.host }}:{{ s.port }}
              </CardDescription>
            </div>
            <div class="flex shrink-0 gap-1">
              <Button
                size="sm"
                variant="outline"
                :disabled="!s.dashboard_url"
                @click="gotoDashboard(s)"
              >
                <Activity class="h-4 w-4" />
                Dashboard
              </Button>
              <Button size="sm" variant="ghost" @click="openLogs(s)">
                <FileText class="h-4 w-4" />
                日志
              </Button>
              <Button size="icon" variant="ghost" title="编辑" @click="openEditServer(s)">
                <Pencil class="h-4 w-4" />
              </Button>
              <Button
                size="icon"
                variant="ghost"
                class="text-destructive hover:bg-destructive/10 hover:text-destructive"
                title="删除"
                @click="askDeleteServer(s)"
              >
                <Trash2 class="h-4 w-4" />
              </Button>
            </div>
          </div>
        </CardHeader>

        <CardContent class="p-0">
          <table v-if="(data.proxiesByServer.get(s.id) ?? []).length > 0" class="w-full text-sm">
            <thead>
              <tr class="border-b bg-muted/30 text-xs uppercase text-muted-foreground">
                <th class="px-4 py-2 text-left font-medium">Name</th>
                <th class="px-4 py-2 text-left font-medium">类型</th>
                <th class="px-4 py-2 text-left font-medium">本地</th>
                <th class="px-4 py-2 text-left font-medium">远端 / 域名</th>
                <th class="px-4 py-2 text-left font-medium">状态</th>
                <th class="px-4 py-2"></th>
              </tr>
            </thead>
            <tbody>
              <tr
                v-for="p in data.proxiesByServer.get(s.id) ?? []"
                :key="p.id"
                class="border-b last:border-b-0 transition-colors hover:bg-muted/40"
              >
                <td class="px-4 py-2">
                  <div class="font-medium font-mono">{{ p.name }}</div>
                  <div
                    v-if="p.description"
                    class="text-xs text-muted-foreground line-clamp-1"
                  >
                    {{ p.description }}
                  </div>
                </td>
                <td class="px-4 py-2">
                  <Badge :variant="proxyTypeStyle(p.proxy_type)" class="font-mono">
                    {{ p.proxy_type }}
                  </Badge>
                </td>
                <td class="px-4 py-2 font-mono text-xs">
                  {{ p.local_ip }}:{{ p.local_port }}
                </td>
                <td class="px-4 py-2 font-mono text-xs text-muted-foreground">
                  <span v-if="p.remote_port">:{{ p.remote_port }}</span>
                  <span v-else-if="p.custom_domains.length">
                    {{ p.custom_domains.join(", ") }}
                  </span>
                  <span v-else>-</span>
                </td>
                <td class="px-4 py-2">
                  <Badge :variant="p.enabled ? 'success' : 'outline'">
                    {{ p.enabled ? "已启用" : "已停用" }}
                  </Badge>
                </td>
                <td class="px-4 py-2 text-right">
                  <div class="flex justify-end gap-1">
                    <Button
                      size="sm"
                      :variant="p.enabled ? 'destructive' : 'default'"
                      @click="toggleProxyRun(p)"
                    >
                      <component :is="p.enabled ? Square : Play" class="h-3.5 w-3.5" />
                      {{ p.enabled ? "停止" : "启动" }}
                    </Button>
                    <Button size="icon" variant="ghost" @click="openEditProxy(p)">
                      <Pencil class="h-4 w-4" />
                    </Button>
                    <Button
                      size="icon"
                      variant="ghost"
                      class="text-destructive hover:bg-destructive/10 hover:text-destructive"
                      @click="askDeleteProxy(p)"
                    >
                      <Trash2 class="h-4 w-4" />
                    </Button>
                  </div>
                </td>
              </tr>
            </tbody>
          </table>

          <div
            v-else
            class="flex items-center justify-between p-4 text-sm text-muted-foreground"
          >
            <span>该服务端下还没有 proxy</span>
            <Button size="sm" variant="outline" @click="openCreateProxy(s)">
              <Plus class="h-4 w-4" />
              添加 Proxy
            </Button>
          </div>

          <div
            v-if="(data.proxiesByServer.get(s.id) ?? []).length > 0"
            class="border-t p-3"
          >
            <Button size="sm" variant="ghost" @click="openCreateProxy(s)">
              <Plus class="h-4 w-4" />
              添加 Proxy
            </Button>
            <Button
              v-if="s.dashboard_url"
              size="sm"
              variant="ghost"
              as="a"
              :href="s.dashboard_url"
              target="_blank"
            >
              <ExternalLink class="h-4 w-4" />
              在浏览器打开 dashboard
            </Button>
          </div>
        </CardContent>
      </Card>
    </div>

    <ServerFormDialog v-model:open="serverFormOpen" :server="editingServer" />
    <ProxyFormDialog
      v-model:open="proxyFormOpen"
      :proxy="editingProxy"
      :default-server-id="proxyDefaultServerId"
    />
    <ProxyLogsDialog
      v-model:open="logsOpen"
      :server="logsServer"
    />
    <ConfirmDialog
      v-model:open="confirmOpen"
      :title="
        pendingDelete?.kind === 'server'
          ? `删除服务端 “${pendingDelete.server.name}” ?`
          : `删除 Proxy “${pendingDelete?.proxy.name ?? ''}” ?`
      "
      :description="
        pendingDelete?.kind === 'server'
          ? '该服务端下的所有 proxy 也会被删除'
          : ''
      "
      confirm-text="删除"
      destructive
      :loading="deleting"
      @confirm="doDelete"
    />
  </div>
</template>
