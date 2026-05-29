<script setup lang="ts">
import { computed, ref } from "vue";
import {
  Card,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { Input } from "@/components/ui/input";
import { Plus, Network, Pencil, Trash2, Play, Square, FileText } from "@lucide/vue";
import ProxyFormDialog from "@/components/forms/ProxyFormDialog.vue";
import ProxyLogsDialog from "@/components/forms/ProxyLogsDialog.vue";
import { ConfirmDialog } from "@/components/ui/confirm";
import { useDataStore } from "@/stores/data";
import { useRuntimeStore } from "@/stores/runtime";
import { toast } from "@/components/ui/toast";
import type { Proxy } from "@/types/store";
import type { ProxyRunStatus } from "@/types/runtime";

const data = useDataStore();
const runtime = useRuntimeStore();

const formOpen = ref(false);
const editing = ref<Proxy | null>(null);

const logsOpen = ref(false);
const logsTarget = ref<Proxy | null>(null);

const confirmOpen = ref(false);
const pendingDelete = ref<Proxy | null>(null);
const deleting = ref(false);

const filter = ref("");

function statusOf(id: string): ProxyRunStatus {
  return runtime.runtimes.get(id)?.status ?? "stopped";
}
function statusBadge(s: ProxyRunStatus) {
  switch (s) {
    case "running":
      return { variant: "success" as const, text: "running" };
    case "starting":
      return { variant: "warning" as const, text: "starting" };
    case "crashed":
      return { variant: "destructive" as const, text: "crashed" };
    default:
      return { variant: "outline" as const, text: "stopped" };
  }
}

async function toggleRun(p: Proxy) {
  const s = statusOf(p.id);
  try {
    if (s === "running" || s === "starting") {
      await runtime.stop(p.id);
      toast.success(`已停止 ${p.name}`);
    } else {
      await runtime.start(p.id);
      toast.success(`已启动 ${p.name}`);
    }
  } catch (e) {
    toast.fromError(e);
  }
}

function openLogs(p: Proxy) {
  logsTarget.value = p;
  logsOpen.value = true;
}

function openCreate() {
  editing.value = null;
  formOpen.value = true;
}
function openEdit(p: Proxy) {
  editing.value = p;
  formOpen.value = true;
}
function askDelete(p: Proxy) {
  pendingDelete.value = p;
  confirmOpen.value = true;
}
async function doDelete() {
  if (!pendingDelete.value) return;
  deleting.value = true;
  try {
    await data.deleteProxy(pendingDelete.value.id);
    toast.success("已删除");
    confirmOpen.value = false;
  } catch (e) {
    toast.fromError(e);
  } finally {
    deleting.value = false;
  }
}

const filtered = computed(() => {
  const q = filter.value.trim().toLowerCase();
  if (!q) return data.proxies;
  return data.proxies.filter(
    (p) =>
      p.name.toLowerCase().includes(q) ||
      p.description.toLowerCase().includes(q) ||
      p.proxy_type.includes(q),
  );
});

function projectName(id: string) {
  return data.projectById.get(id)?.name ?? "(deleted)";
}
function serverName(id: string) {
  return data.serverById.get(id)?.name ?? "(deleted)";
}

const canCreate = computed(
  () => data.projects.length > 0 && data.servers.length > 0,
);
</script>

<template>
  <div class="space-y-4">
    <div class="flex items-center justify-between gap-4">
      <div>
        <h2 class="text-2xl font-semibold tracking-tight">Proxy 列表</h2>
        <p class="text-sm text-muted-foreground">所有已配置的代理（启停将在下一步实现）</p>
      </div>
      <Button :disabled="!canCreate" @click="openCreate">
        <Plus class="h-4 w-4" />
        新建 Proxy
      </Button>
    </div>

    <Card v-if="!canCreate">
      <CardHeader>
        <CardTitle>需要先准备好项目和服务端</CardTitle>
        <CardDescription>
          创建 Proxy 前需要至少 1 个项目 和 1 个 frps 服务端
        </CardDescription>
      </CardHeader>
    </Card>

    <Card v-else-if="data.proxies.length === 0">
      <CardHeader class="flex flex-col items-center text-center">
        <div class="mx-auto mb-2 flex h-12 w-12 items-center justify-center rounded-full bg-secondary">
          <Network class="h-6 w-6 text-muted-foreground" />
        </div>
        <CardTitle>还没有 Proxy</CardTitle>
        <CardDescription>到「项目」页面或者用上面的按钮新建一个</CardDescription>
      </CardHeader>
    </Card>

    <template v-else>
      <Input v-model="filter" placeholder="过滤 name / 类型 / 描述" class="max-w-md" />

      <Card class="p-0">
        <div class="overflow-x-auto">
          <table class="w-full text-sm">
            <thead>
              <tr class="border-b bg-muted/30 text-xs uppercase text-muted-foreground">
                <th class="px-4 py-2 text-left font-medium">Name</th>
                <th class="px-4 py-2 text-left font-medium">类型</th>
                <th class="px-4 py-2 text-left font-medium">本地</th>
                <th class="px-4 py-2 text-left font-medium">远端 / 域名</th>
                <th class="px-4 py-2 text-left font-medium">项目 / 服务端</th>
                <th class="px-4 py-2 text-left font-medium">运行</th>
                <th class="px-4 py-2"></th>
              </tr>
            </thead>
            <tbody>
              <tr
                v-for="p in filtered"
                :key="p.id"
                class="border-b transition-colors hover:bg-muted/40"
              >
                <td class="px-4 py-2">
                  <div class="font-medium font-mono">{{ p.name }}</div>
                  <div class="text-xs text-muted-foreground line-clamp-1">{{ p.description }}</div>
                </td>
                <td class="px-4 py-2">
                  <Badge variant="secondary" class="font-mono">{{ p.proxy_type }}</Badge>
                </td>
                <td class="px-4 py-2 font-mono text-xs">{{ p.local_ip }}:{{ p.local_port }}</td>
                <td class="px-4 py-2 font-mono text-xs text-muted-foreground">
                  <span v-if="p.remote_port">:{{ p.remote_port }}</span>
                  <span v-else-if="p.custom_domains.length">{{ p.custom_domains.join(", ") }}</span>
                  <span v-else>-</span>
                </td>
                <td class="px-4 py-2 text-xs">
                  <div>{{ projectName(p.project_id) }}</div>
                  <div class="text-muted-foreground">{{ serverName(p.server_id) }}</div>
                </td>
                <td class="px-4 py-2">
                  <div class="flex items-center gap-2">
                    <Badge :variant="statusBadge(statusOf(p.id)).variant">
                      {{ statusBadge(statusOf(p.id)).text }}
                    </Badge>
                    <Button
                      size="sm"
                      :variant="
                        statusOf(p.id) === 'running' || statusOf(p.id) === 'starting'
                          ? 'destructive'
                          : 'default'
                      "
                      @click="toggleRun(p)"
                    >
                      <component
                        :is="
                          statusOf(p.id) === 'running' || statusOf(p.id) === 'starting'
                            ? Square
                            : Play
                        "
                        class="h-3.5 w-3.5"
                      />
                      {{
                        statusOf(p.id) === 'running' || statusOf(p.id) === 'starting'
                          ? '停止'
                          : '启动'
                      }}
                    </Button>
                  </div>
                </td>
                <td class="px-4 py-2 text-right">
                  <div class="flex justify-end gap-1">
                    <Button size="icon" variant="ghost" title="日志" @click="openLogs(p)">
                      <FileText class="h-4 w-4" />
                    </Button>
                    <Button size="icon" variant="ghost" @click="openEdit(p)">
                      <Pencil class="h-4 w-4" />
                    </Button>
                    <Button
                      size="icon"
                      variant="ghost"
                      class="text-destructive hover:bg-destructive/10 hover:text-destructive"
                      @click="askDelete(p)"
                    >
                      <Trash2 class="h-4 w-4" />
                    </Button>
                  </div>
                </td>
              </tr>
              <tr v-if="filtered.length === 0">
                <td colspan="7" class="p-8 text-center text-muted-foreground">
                  没有匹配的条目
                </td>
              </tr>
            </tbody>
          </table>
        </div>
      </Card>
    </template>

    <ProxyFormDialog v-model:open="formOpen" :proxy="editing" />
    <ProxyLogsDialog v-model:open="logsOpen" :proxy="logsTarget" />
    <ConfirmDialog
      v-model:open="confirmOpen"
      :title="`删除 Proxy “${pendingDelete?.name ?? ''}” ?`"
      confirm-text="删除"
      destructive
      :loading="deleting"
      @confirm="doDelete"
    />
  </div>
</template>
