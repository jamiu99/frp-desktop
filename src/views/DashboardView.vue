<script setup lang="ts">
import { computed, onMounted, ref, watch } from "vue";
import { useRoute } from "vue-router";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { Input } from "@/components/ui/input";
import {
  RefreshCw,
  Server,
  ArrowUp,
  ArrowDown,
  ArrowUpDown,
} from "@lucide/vue";
import { useDataStore } from "@/stores/data";
import { frpsApi, type FrpsProxyInfo, type FrpsServerInfo } from "@/api/frps";
import { toast } from "@/components/ui/toast";
import { cn } from "@/lib/utils";

const route = useRoute();
const data = useDataStore();

const serverId = computed(() => route.params.serverId as string);
const server = computed(() => data.servers.find((s) => s.id === serverId.value));

const info = ref<FrpsServerInfo | null>(null);
const proxies = ref<(FrpsProxyInfo & { type: string })[]>([]);
const loading = ref(false);
const error = ref<string | null>(null);
const filter = ref("");

type SortKey = "name" | "type" | "status" | "cur_conns" | "today_traffic_in" | "today_traffic_out";
const sortKey = ref<SortKey>("name");
const sortDir = ref<"asc" | "desc">("asc");

async function refresh() {
  if (!server.value) return;
  loading.value = true;
  error.value = null;
  try {
    const [si, ps] = await Promise.all([
      frpsApi.serverInfo(server.value),
      frpsApi.allProxies(server.value),
    ]);
    info.value = si;
    proxies.value = ps;
  } catch (e) {
    error.value = String((e as Error)?.message ?? e);
    toast.fromError(e);
  } finally {
    loading.value = false;
  }
}

onMounted(() => {
  if (data.loaded) refresh();
});
watch(
  () => [data.loaded, server.value?.id],
  () => {
    if (data.loaded && server.value) refresh();
  },
  { immediate: true },
);

function formatBytes(n: number) {
  if (n < 1024) return `${n} B`;
  if (n < 1024 * 1024) return `${(n / 1024).toFixed(1)} KB`;
  if (n < 1024 * 1024 * 1024) return `${(n / 1024 / 1024).toFixed(1)} MB`;
  return `${(n / 1024 / 1024 / 1024).toFixed(2)} GB`;
}

function toggleSort(k: SortKey) {
  if (sortKey.value === k) {
    sortDir.value = sortDir.value === "asc" ? "desc" : "asc";
  } else {
    sortKey.value = k;
    sortDir.value = "asc";
  }
}

function sortIcon(k: SortKey) {
  if (sortKey.value !== k) return ArrowUpDown;
  return sortDir.value === "asc" ? ArrowUp : ArrowDown;
}

const filteredSorted = computed(() => {
  const q = filter.value.trim().toLowerCase();
  let list = proxies.value;
  if (q) {
    list = list.filter(
      (p) =>
        p.name.toLowerCase().includes(q) ||
        p.type.toLowerCase().includes(q) ||
        p.status.toLowerCase().includes(q),
    );
  }
  const k = sortKey.value;
  const factor = sortDir.value === "asc" ? 1 : -1;
  return [...list].sort((a, b) => {
    const av = (a as never)[k];
    const bv = (b as never)[k];
    if (typeof av === "number" && typeof bv === "number")
      return (av - bv) * factor;
    return String(av).localeCompare(String(bv)) * factor;
  });
});

const cols: { key: SortKey; label: string }[] = [
  { key: "name", label: "Name" },
  { key: "type", label: "类型" },
  { key: "status", label: "状态" },
  { key: "cur_conns", label: "当前连接" },
  { key: "today_traffic_in", label: "今日入站" },
  { key: "today_traffic_out", label: "今日出站" },
];
</script>

<template>
  <div v-if="!server" class="text-sm text-muted-foreground">
    服务端不存在或已被删除。
  </div>

  <div v-else class="space-y-4">
    <div class="flex items-start justify-between gap-4">
      <div class="min-w-0">
        <h2 class="flex items-center gap-2 text-2xl font-semibold tracking-tight">
          <Server class="h-5 w-5 text-primary" />
          {{ server.name }} dashboard
        </h2>
        <p class="text-sm text-muted-foreground">
          {{ server.dashboard_url || "未配置 dashboard URL" }}
        </p>
      </div>
      <Button :disabled="loading || !server.dashboard_url" variant="outline" @click="refresh">
        <RefreshCw :class="cn('h-4 w-4', loading && 'animate-spin')" />
        刷新
      </Button>
    </div>

    <div v-if="!server.dashboard_url" class="rounded-md border border-dashed p-6 text-center text-sm text-muted-foreground">
      该服务端未配置 dashboard URL，无法查看运行状态
    </div>

    <div v-if="error" class="rounded-md border border-destructive/40 bg-destructive/5 p-3 text-sm text-destructive">
      {{ error }}
    </div>

    <div v-if="info" class="grid grid-cols-2 gap-3 md:grid-cols-4">
      <Card>
        <CardHeader class="pb-2">
          <CardDescription>frp 版本</CardDescription>
          <CardTitle class="text-base font-mono">{{ info.version }}</CardTitle>
        </CardHeader>
      </Card>
      <Card>
        <CardHeader class="pb-2">
          <CardDescription>客户端数</CardDescription>
          <CardTitle class="text-base">{{ info.client_counts }}</CardTitle>
        </CardHeader>
      </Card>
      <Card>
        <CardHeader class="pb-2">
          <CardDescription>当前连接</CardDescription>
          <CardTitle class="text-base">{{ info.cur_conns }}</CardTitle>
        </CardHeader>
      </Card>
      <Card>
        <CardHeader class="pb-2">
          <CardDescription>累计入站 / 出站</CardDescription>
          <CardTitle class="text-sm font-mono">
            {{ formatBytes(info.total_traffic_in) }}
            <span class="text-muted-foreground">/</span>
            {{ formatBytes(info.total_traffic_out) }}
          </CardTitle>
        </CardHeader>
      </Card>
    </div>

    <Card v-if="server.dashboard_url">
      <CardHeader class="gap-3">
        <div class="flex items-center justify-between">
          <CardTitle class="text-base">Proxy（{{ filteredSorted.length }} / {{ proxies.length }}）</CardTitle>
          <CardDescription class="text-xs">点击列头排序</CardDescription>
        </div>
        <Input v-model="filter" placeholder="过滤 name / 类型 / 状态" class="max-w-md" />
      </CardHeader>
      <CardContent class="p-0">
        <div class="overflow-x-auto">
          <table class="w-full text-sm">
            <thead>
              <tr class="border-y bg-muted/30 text-xs uppercase text-muted-foreground">
                <th
                  v-for="c in cols"
                  :key="c.key"
                  class="cursor-pointer select-none whitespace-nowrap px-4 py-2 text-left font-medium hover:text-foreground"
                  @click="toggleSort(c.key)"
                >
                  <span class="inline-flex items-center gap-1">
                    {{ c.label }}
                    <component :is="sortIcon(c.key)" class="h-3 w-3" />
                  </span>
                </th>
              </tr>
            </thead>
            <tbody>
              <tr v-if="!loading && filteredSorted.length === 0" class="border-b">
                <td colspan="6" class="p-8 text-center text-muted-foreground">
                  {{ filter ? "没有匹配" : "服务端没有 proxy" }}
                </td>
              </tr>
              <tr
                v-for="p in filteredSorted"
                :key="`${p.type}-${p.name}`"
                class="border-b transition-colors hover:bg-muted/40"
              >
                <td class="px-4 py-2 font-medium">{{ p.name }}</td>
                <td class="px-4 py-2">
                  <Badge variant="secondary" class="font-mono">{{ p.type }}</Badge>
                </td>
                <td class="px-4 py-2">
                  <Badge
                    :variant="p.status === 'online' ? 'success' : 'outline'"
                  >{{ p.status }}</Badge>
                </td>
                <td class="px-4 py-2 tabular-nums">{{ p.cur_conns }}</td>
                <td class="px-4 py-2 tabular-nums font-mono text-xs">
                  {{ formatBytes(p.today_traffic_in) }}
                </td>
                <td class="px-4 py-2 tabular-nums font-mono text-xs">
                  {{ formatBytes(p.today_traffic_out) }}
                </td>
              </tr>
            </tbody>
          </table>
        </div>
      </CardContent>
    </Card>
  </div>
</template>
