<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Badge } from "@/components/ui/badge";
import { ArrowUp, ArrowDown, ArrowUpDown, RefreshCw } from "@lucide/vue";
import { cn } from "@/lib/utils";

interface PortEntry {
  process: string;
  pid: number;
  protocol: string;
  local_address: string;
  local_port: number;
  state: string | null;
}

type SortKey = keyof PortEntry;
type SortDir = "asc" | "desc";

const entries = ref<PortEntry[]>([]);
const loading = ref(false);
const error = ref<string | null>(null);
const filter = ref("");
const sortKey = ref<SortKey>("local_port");
const sortDir = ref<SortDir>("asc");

async function refresh() {
  loading.value = true;
  error.value = null;
  try {
    entries.value = await invoke<PortEntry[]>("list_ports");
  } catch (e) {
    error.value = String(e);
  } finally {
    loading.value = false;
  }
}

onMounted(refresh);

function toggleSort(key: SortKey) {
  if (sortKey.value === key) {
    sortDir.value = sortDir.value === "asc" ? "desc" : "asc";
  } else {
    sortKey.value = key;
    sortDir.value = "asc";
  }
}

function compareValues(a: unknown, b: unknown): number {
  if (a == null && b == null) return 0;
  if (a == null) return -1;
  if (b == null) return 1;
  if (typeof a === "number" && typeof b === "number") return a - b;
  return String(a).localeCompare(String(b), "zh-CN", { numeric: true });
}

const filteredSorted = computed(() => {
  const q = filter.value.trim().toLowerCase();
  let list = entries.value;
  if (q) {
    list = list.filter((e) => {
      return (
        e.process.toLowerCase().includes(q) ||
        String(e.pid).includes(q) ||
        e.protocol.toLowerCase().includes(q) ||
        e.local_address.toLowerCase().includes(q) ||
        String(e.local_port).includes(q) ||
        (e.state ?? "").toLowerCase().includes(q)
      );
    });
  }
  const k = sortKey.value;
  const factor = sortDir.value === "asc" ? 1 : -1;
  return [...list].sort((a, b) => compareValues(a[k], b[k]) * factor);
});

const columns: { key: SortKey; label: string; class?: string }[] = [
  { key: "process", label: "进程" },
  { key: "pid", label: "PID", class: "tabular-nums" },
  { key: "protocol", label: "协议" },
  { key: "local_address", label: "本地地址" },
  { key: "local_port", label: "本地端口", class: "tabular-nums" },
  { key: "state", label: "状态" },
];

function sortIcon(key: SortKey) {
  if (sortKey.value !== key) return ArrowUpDown;
  return sortDir.value === "asc" ? ArrowUp : ArrowDown;
}

function protoVariant(p: string) {
  if (p.startsWith("tcp")) return "default" as const;
  return "secondary" as const;
}
</script>

<template>
  <div class="space-y-4">
    <div class="flex items-start justify-between gap-4">
      <p class="text-sm text-muted-foreground">
        当前电脑上所有 TCP/UDP socket。配置 frpc 时可以来这里查端口。
      </p>
      <Button class="shrink-0" :disabled="loading" variant="outline" @click="refresh">
        <RefreshCw :class="cn('h-4 w-4', loading && 'animate-spin')" />
        刷新
      </Button>
    </div>

    <Card>
      <CardHeader class="gap-3">
        <div class="flex items-center justify-between">
          <CardTitle class="text-base">
            共 {{ filteredSorted.length }} 条 (总 {{ entries.length }})
          </CardTitle>
          <CardDescription class="text-xs">
            点击列表头切换排序方向
          </CardDescription>
        </div>
        <Input
          v-model="filter"
          placeholder="过滤：进程名 / PID / 端口 / 地址 ..."
          class="max-w-md"
        />
      </CardHeader>
      <CardContent class="p-0">
        <div v-if="error" class="border-t p-4 text-sm text-destructive">
          加载失败：{{ error }}
        </div>

        <div class="overflow-x-auto">
          <table class="w-full text-sm">
            <thead>
              <tr class="border-y bg-muted/30 text-xs uppercase text-muted-foreground">
                <th
                  v-for="col in columns"
                  :key="col.key"
                  scope="col"
                  class="cursor-pointer select-none whitespace-nowrap px-4 py-2 text-left font-medium hover:text-foreground"
                  @click="toggleSort(col.key)"
                >
                  <span class="inline-flex items-center gap-1">
                    {{ col.label }}
                    <component :is="sortIcon(col.key)" class="h-3 w-3" />
                  </span>
                </th>
              </tr>
            </thead>
            <tbody>
              <tr
                v-if="!loading && filteredSorted.length === 0"
                class="border-b"
              >
                <td colspan="6" class="p-8 text-center text-muted-foreground">
                  {{ filter ? "没有匹配的条目" : "没有 socket" }}
                </td>
              </tr>
              <tr
                v-for="(row, i) in filteredSorted"
                :key="`${row.protocol}-${row.local_address}-${row.local_port}-${row.pid}-${i}`"
                class="border-b transition-colors hover:bg-muted/40"
              >
                <td class="px-4 py-2 font-medium">
                  {{ row.process }}
                </td>
                <td class="px-4 py-2 tabular-nums text-muted-foreground">
                  {{ row.pid || "-" }}
                </td>
                <td class="px-4 py-2">
                  <Badge :variant="protoVariant(row.protocol)" class="font-mono">
                    {{ row.protocol }}
                  </Badge>
                </td>
                <td class="px-4 py-2 font-mono text-xs text-muted-foreground">
                  {{ row.local_address }}
                </td>
                <td class="px-4 py-2 tabular-nums font-mono">
                  {{ row.local_port }}
                </td>
                <td class="px-4 py-2 text-xs text-muted-foreground">
                  {{ row.state ?? "-" }}
                </td>
              </tr>
            </tbody>
          </table>
        </div>
      </CardContent>
    </Card>
  </div>
</template>
