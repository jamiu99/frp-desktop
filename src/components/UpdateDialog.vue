<script setup lang="ts">
import { computed } from "vue";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Button } from "@/components/ui/button";
import { Download, RotateCw } from "@lucide/vue";
import { useUpdater } from "@/composables/useUpdater";
import { toast } from "@/components/ui/toast";

defineProps<{ open: boolean }>();
const emit = defineEmits<{ (e: "update:open", v: boolean): void }>();

const {
  phase,
  currentUpdate,
  downloaded,
  total,
  downloadAndInstall,
  restartApp,
} = useUpdater();

const downloading = computed(() => phase.value === "downloading");
const ready = computed(() => phase.value === "ready");

const progressText = computed(() => {
  if (phase.value !== "downloading") return "";
  if (!total.value) return `已下载 ${(downloaded.value / 1024 / 1024).toFixed(1)} MB`;
  const pct = Math.floor((downloaded.value / total.value) * 100);
  return `${pct}%（${(downloaded.value / 1024 / 1024).toFixed(1)} / ${(total.value / 1024 / 1024).toFixed(1)} MB）`;
});

const progressPct = computed(() => {
  if (phase.value !== "downloading" || !total.value) return 0;
  return Math.floor((downloaded.value / total.value) * 100);
});

async function doUpdate() {
  try {
    await downloadAndInstall();
  } catch (e) {
    toast.fromError(e);
  }
}

async function doRestart() {
  try {
    await restartApp();
  } catch (e) {
    toast.fromError(e);
  }
}

function later() {
  emit("update:open", false);
}
</script>

<template>
  <Dialog :open="open" @update:open="emit('update:open', $event)">
    <DialogContent class="max-w-md">
      <DialogHeader>
        <DialogTitle>发现新版本 {{ currentUpdate?.version }}</DialogTitle>
        <DialogDescription>
          当前版本 {{ currentUpdate?.currentVersion }}，可更新到
          {{ currentUpdate?.version }}
        </DialogDescription>
      </DialogHeader>

      <div
        v-if="currentUpdate?.body"
        class="max-h-48 overflow-y-auto whitespace-pre-wrap rounded-md border bg-muted/40 p-3 text-xs leading-relaxed"
      >
        {{ currentUpdate.body }}
      </div>

      <div v-if="downloading" class="space-y-2">
        <div class="h-2 w-full overflow-hidden rounded-full bg-secondary">
          <div
            class="h-full bg-primary transition-all"
            :style="{ width: progressPct + '%' }"
          />
        </div>
        <p class="text-xs text-muted-foreground">{{ progressText }}</p>
      </div>

      <DialogFooter>
        <template v-if="ready">
          <Button variant="outline" @click="later">稍后重启</Button>
          <Button @click="doRestart">
            <RotateCw class="h-4 w-4" />
            立即重启
          </Button>
        </template>
        <template v-else>
          <Button variant="outline" :disabled="downloading" @click="later">
            稍后
          </Button>
          <Button :disabled="downloading" @click="doUpdate">
            <Download class="h-4 w-4" />
            {{ downloading ? "下载中…" : "立即更新" }}
          </Button>
        </template>
      </DialogFooter>
    </DialogContent>
  </Dialog>
</template>
