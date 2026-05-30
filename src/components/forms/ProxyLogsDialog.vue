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
import { STATUS_LABEL } from "@/types/runtime";
import type { FrpsServer } from "@/types/store";

const props = defineProps<{
  open: boolean;
  server: FrpsServer | null;
}>();
const emit = defineEmits<{
  (e: "update:open", v: boolean): void;
}>();

const runtime = useRuntimeStore();
const logBox = ref<HTMLDivElement | null>(null);
const autoScroll = ref(true);

const lines = computed(() => {
  if (!props.server) return [] as string[];
  return runtime.runtimes.get(props.server.id)?.log_tail ?? [];
});

const status = computed(() => {
  if (!props.server) return STATUS_LABEL.stopped;
  return STATUS_LABEL[runtime.statusOfServer(props.server.id)];
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
        <DialogTitle>日志 — {{ server?.name ?? "" }}</DialogTitle>
        <DialogDescription>
          状态: {{ status }} ｜ 共 {{ lines.length }} 行（最多保留 500 行）
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
          暂无日志（启动该服务端的 proxy 后这里会有输出）
        </div>
        <div v-for="(l, i) in lines" :key="i">{{ l }}</div>
      </div>
    </DialogContent>
  </Dialog>
</template>
