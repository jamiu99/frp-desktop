<script setup lang="ts">
import { computed, ref } from "vue";
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
} from "@lucide/vue";
import ServerFormDialog from "@/components/forms/ServerFormDialog.vue";
import { ConfirmDialog } from "@/components/ui/confirm";
import { useDataStore } from "@/stores/data";
import { toast } from "@/components/ui/toast";
import type { FrpsServer } from "@/types/store";

const data = useDataStore();
const router = useRouter();

const formOpen = ref(false);
const editing = ref<FrpsServer | null>(null);

const confirmOpen = ref(false);
const pendingDelete = ref<FrpsServer | null>(null);
const deleting = ref(false);

function openCreate() {
  editing.value = null;
  formOpen.value = true;
}
function openEdit(s: FrpsServer) {
  editing.value = s;
  formOpen.value = true;
}
function askDelete(s: FrpsServer) {
  pendingDelete.value = s;
  confirmOpen.value = true;
}
async function doDelete() {
  if (!pendingDelete.value) return;
  deleting.value = true;
  try {
    await data.deleteServer(pendingDelete.value.id);
    toast.success("已删除");
    confirmOpen.value = false;
  } catch (e) {
    toast.fromError(e);
  } finally {
    deleting.value = false;
  }
}

const proxiesByServer = computed(() => {
  const m = new Map<string, number>();
  for (const p of data.proxies) {
    m.set(p.server_id, (m.get(p.server_id) ?? 0) + 1);
  }
  return m;
});

function gotoDashboard(s: FrpsServer) {
  router.push({ name: "dashboard", params: { serverId: s.id } });
}
</script>

<template>
  <div class="space-y-6">
    <div class="flex items-center justify-between">
      <div>
        <h2 class="text-2xl font-semibold tracking-tight">frps 服务端</h2>
        <p class="text-sm text-muted-foreground">
          配置一个或多个 frps 服务端，用于客户端连接和 dashboard 查看
        </p>
      </div>
      <Button @click="openCreate">
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
        <CardDescription>
          添加一个 frps 服务端，例如 frps.example.com:7000
        </CardDescription>
      </CardHeader>
      <CardContent class="flex justify-center pb-6">
        <Button variant="outline" @click="openCreate">
          <Plus class="h-4 w-4" />
          添加第一个
        </Button>
      </CardContent>
    </Card>

    <div v-else class="grid grid-cols-1 gap-4 lg:grid-cols-2">
      <Card v-for="s in data.servers" :key="s.id">
        <CardHeader>
          <div class="flex items-start justify-between gap-2">
            <div class="min-w-0">
              <CardTitle class="flex items-center gap-2">
                <Server class="h-4 w-4 text-primary shrink-0" />
                <span class="truncate">{{ s.name }}</span>
              </CardTitle>
              <CardDescription class="mt-1 font-mono text-xs">
                {{ s.host }}:{{ s.port }}
              </CardDescription>
            </div>
            <div class="flex gap-1">
              <Button
                size="icon"
                variant="ghost"
                title="编辑"
                @click="openEdit(s)"
              >
                <Pencil class="h-4 w-4" />
              </Button>
              <Button
                size="icon"
                variant="ghost"
                class="text-destructive hover:bg-destructive/10 hover:text-destructive"
                title="删除"
                @click="askDelete(s)"
              >
                <Trash2 class="h-4 w-4" />
              </Button>
            </div>
          </div>
        </CardHeader>
        <CardContent class="space-y-3">
          <div class="flex flex-wrap items-center gap-2 text-xs">
            <Badge variant="secondary">
              {{ proxiesByServer.get(s.id) ?? 0 }} proxies
            </Badge>
            <Badge v-if="s.dashboard_url" variant="outline">
              dashboard
            </Badge>
          </div>
          <div class="flex flex-wrap gap-2">
            <Button
              size="sm"
              variant="outline"
              :disabled="!s.dashboard_url"
              @click="gotoDashboard(s)"
            >
              <Activity class="h-4 w-4" />
              查看 dashboard
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
              浏览器打开
            </Button>
          </div>
        </CardContent>
      </Card>
    </div>

    <ServerFormDialog v-model:open="formOpen" :server="editing" />
    <ConfirmDialog
      v-model:open="confirmOpen"
      :title="`删除服务端 “${pendingDelete?.name ?? ''}” ?`"
      description="如果服务端下还有 proxy，会被拒绝。删除前请先迁移或删除 proxy。"
      confirm-text="删除"
      destructive
      :loading="deleting"
      @confirm="doDelete"
    />
  </div>
</template>
