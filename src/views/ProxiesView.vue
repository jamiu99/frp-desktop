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
import { Plus, Network, Pencil, Trash2, Play, Square } from "@lucide/vue";
import ProxyFormDialog from "@/components/forms/ProxyFormDialog.vue";
import { ConfirmDialog } from "@/components/ui/confirm";
import { useDataStore } from "@/stores/data";
import { useRuntimeStore } from "@/stores/runtime";
import { toast } from "@/components/ui/toast";
import type { Proxy } from "@/types/store";

const data = useDataStore();
const runtime = useRuntimeStore();

const formOpen = ref(false);
const editing = ref<Proxy | null>(null);

const confirmOpen = ref(false);
const pendingDelete = ref<Proxy | null>(null);
const deleting = ref(false);

const filter = ref("");

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

async function toggleRun(p: Proxy) {
  try {
    if (p.enabled) {
      await runtime.stopProxy(p.id);
      await data.fetchAll();
    } else {
      await runtime.startProxy(p.id);
      await data.fetchAll();
    }
  } catch (e) {
    toast.fromError(e);
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

function serverName(id: string) {
  return data.serverById.get(id)?.name ?? "(已删除)";
}

const canCreate = computed(() => data.servers.length > 0);
</script>

<template>
  <div class="space-y-4">
    <div class="flex items-center justify-between gap-4">
      <p class="text-sm text-muted-foreground">
        所有 proxy 的扁平视图。按服务端分组的视图见「frps 服务端」页
      </p>
      <Button class="shrink-0" :disabled="!canCreate" @click="openCreate">
        <Plus class="h-4 w-4" />
        新建 Proxy
      </Button>
    </div>

    <Card v-if="!canCreate">
      <CardHeader>
        <CardTitle>需要先添加 frps 服务端</CardTitle>
        <CardDescription>到「frps 服务端」页添加一个</CardDescription>
      </CardHeader>
    </Card>

    <Card v-else-if="data.proxies.length === 0">
      <CardHeader class="flex flex-col items-center text-center">
        <div class="mx-auto mb-2 flex h-12 w-12 items-center justify-center rounded-full bg-secondary">
          <Network class="h-6 w-6 text-muted-foreground" />
        </div>
        <CardTitle>还没有 Proxy</CardTitle>
        <CardDescription>点上方「新建 Proxy」开始</CardDescription>
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
                <th class="px-4 py-2 text-left font-medium">服务端</th>
                <th class="px-4 py-2 text-left font-medium">状态</th>
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
                  <div v-if="p.description" class="text-xs text-muted-foreground line-clamp-1">
                    {{ p.description }}
                  </div>
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
                  {{ serverName(p.server_id) }}
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
                      @click="toggleRun(p)"
                    >
                      <component :is="p.enabled ? Square : Play" class="h-3.5 w-3.5" />
                      {{ p.enabled ? "停止" : "启动" }}
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
