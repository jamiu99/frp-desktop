<script setup lang="ts">
import { ref, watch } from "vue";
import {
  Dialog,
  DialogClose,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Button } from "@/components/ui/button";
import { storeApi } from "@/api/store";
import { useDataStore } from "@/stores/data";
import { useRuntimeStore } from "@/stores/runtime";
import { toast } from "@/components/ui/toast";
import type { ImportMode, ImportReport } from "@/types/store";

const props = defineProps<{
  open: boolean;
  /** 已选中的导入文件路径 */
  path: string | null;
}>();
const emit = defineEmits<{
  (e: "update:open", v: boolean): void;
}>();

const data = useDataStore();
const runtime = useRuntimeStore();

const mode = ref<ImportMode>("merge");
const preview = ref<ImportReport | null>(null);
const previewError = ref<string | null>(null);
const importing = ref(false);

function errText(e: unknown) {
  return typeof e === "string" ? e : (e as Error)?.message ?? String(e);
}

// 打开或切换模式时做一次 dry-run 预览
watch(
  () => [props.open, props.path, mode.value] as const,
  async ([open, path, m]) => {
    if (!open || !path) return;
    preview.value = null;
    previewError.value = null;
    try {
      preview.value = await storeApi.importConfig(path, m, true);
    } catch (e) {
      previewError.value = errText(e);
    }
  },
  { immediate: true },
);

watch(
  () => props.open,
  (open) => {
    if (open) mode.value = "merge";
  },
);

async function doImport() {
  if (!props.path) return;
  importing.value = true;
  try {
    const r = await storeApi.importConfig(props.path, mode.value, false);
    await data.fetchAll();
    await runtime.refresh();
    toast.success(
      `导入完成：服务端 +${r.servers_added} / 更新 ${r.servers_updated}，proxy +${r.proxies_added} / 更新 ${r.proxies_updated}`,
    );
    emit("update:open", false);
  } catch (e) {
    toast.fromError(e);
  } finally {
    importing.value = false;
  }
}

function fileName(p: string | null) {
  return p ? p.split(/[\\/]/).pop() : "";
}
</script>

<template>
  <Dialog :open="open" @update:open="emit('update:open', $event)">
    <DialogContent class="max-w-md">
      <DialogHeader>
        <DialogTitle>导入配置</DialogTitle>
        <DialogDescription class="break-all font-mono text-xs">
          {{ fileName(path) }}
        </DialogDescription>
      </DialogHeader>

      <div class="space-y-4 text-sm">
        <div class="space-y-2">
          <label class="flex items-start gap-2">
            <input v-model="mode" type="radio" value="merge" class="mt-1" />
            <span>
              <span class="font-medium">合并</span>
              <span class="block text-xs text-muted-foreground">
                同名服务端、同服务端下同名 proxy 用文件内容更新，其余新增；现有配置不删除
              </span>
            </span>
          </label>
          <label class="flex items-start gap-2">
            <input v-model="mode" type="radio" value="replace" class="mt-1" />
            <span>
              <span class="font-medium">覆盖</span>
              <span class="block text-xs text-muted-foreground">
                清空现有全部服务端和 proxy（正在运行的会被停止），完全以文件为准
              </span>
            </span>
          </label>
        </div>

        <div
          v-if="previewError"
          class="rounded-md border border-destructive/50 p-3 text-xs text-destructive"
        >
          {{ previewError }}
        </div>
        <div v-else-if="preview" class="rounded-md border p-3 text-xs">
          <div class="mb-1 font-medium">将会：</div>
          <ul class="space-y-0.5 text-muted-foreground">
            <li v-if="preview.servers_removed || preview.proxies_removed" class="text-destructive">
              删除 {{ preview.servers_removed }} 个服务端、{{ preview.proxies_removed }} 个 proxy
            </li>
            <li>新增 {{ preview.servers_added }} 个服务端、{{ preview.proxies_added }} 个 proxy</li>
            <li>更新 {{ preview.servers_updated }} 个服务端、{{ preview.proxies_updated }} 个 proxy</li>
          </ul>
          <p class="mt-2 text-muted-foreground">
            新导入的 proxy 默认为停用，需手动启动（避免和其他机器抢同一个端口）
          </p>
        </div>
        <div v-else class="text-xs text-muted-foreground">正在读取文件…</div>
      </div>

      <DialogFooter>
        <DialogClose as-child>
          <Button type="button" variant="outline" :disabled="importing">取消</Button>
        </DialogClose>
        <Button
          type="button"
          :variant="mode === 'replace' ? 'destructive' : 'default'"
          :disabled="importing || !preview"
          @click="doImport"
        >
          {{ importing ? "导入中…" : mode === "replace" ? "覆盖导入" : "合并导入" }}
        </Button>
      </DialogFooter>
    </DialogContent>
  </Dialog>
</template>
