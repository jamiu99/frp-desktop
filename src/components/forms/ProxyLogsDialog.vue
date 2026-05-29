<script setup lang="ts">
import { computed, nextTick, ref, watch } from "vue";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Button } from "@/components/ui/button";
import { useRuntimeStore } from "@/stores/runtime";
import type { Proxy } from "@/types/store";

const props = defineProps<{
  open: boolean;
  proxy: Proxy | null;
}>();
const emit = defineEmits<{
  (e: "update:open", v: boolean): void;
}>();

const runtime = useRuntimeStore();
const logBox = ref<HTMLDivElement | null>(null);
const autoScroll = ref(true);

const lines = computed(() => {
  if (!props.proxy) return [] as string[];
  return runtime.runtimes.get(props.proxy.id)?.log_tail ?? [];
});

watch(lines, async () => {
  if (autoScroll.value) {
    await nextTick();
    if (logBox.value) {
      logBox.value.scrollTop = logBox.value.scrollHeight;
    }
  }
});
</script>

<template>
  <Dialog :open="open" @update:open="emit('update:open', $event)">
    <DialogContent class="max-w-3xl">
      <DialogHeader>
        <DialogTitle>日志 — {{ proxy?.name ?? "" }}</DialogTitle>
        <DialogDescription>
          frpc 子进程的最近 {{ lines.length }} 行（最多保留 500 行）
        </DialogDescription>
      </DialogHeader>

      <div class="flex items-center justify-between text-xs">
        <label class="inline-flex items-center gap-2">
          <input v-model="autoScroll" type="checkbox" />
          自动滚到底部
        </label>
        <Button size="sm" variant="ghost" @click="emit('update:open', false)">关闭</Button>
      </div>

      <div
        ref="logBox"
        class="h-80 overflow-y-auto rounded-md border bg-muted/40 p-3 font-mono text-xs leading-relaxed"
      >
        <div v-if="lines.length === 0" class="text-muted-foreground">
          暂无日志（启动 proxy 后这里会有输出）
        </div>
        <div v-for="(l, i) in lines" :key="i">{{ l }}</div>
      </div>
    </DialogContent>
  </Dialog>
</template>
