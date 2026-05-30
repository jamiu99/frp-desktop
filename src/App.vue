<script setup lang="ts">
import { onMounted, ref } from "vue";
import AppSidebar from "@/components/layout/AppSidebar.vue";
import AppTopbar from "@/components/layout/AppTopbar.vue";
import UpdateDialog from "@/components/UpdateDialog.vue";
import { Toaster } from "@/components/ui/toast";
import { useDataStore } from "@/stores/data";
import { useRuntimeStore } from "@/stores/runtime";
import { useUpdater } from "@/composables/useUpdater";
import { toast } from "@/components/ui/toast";

const data = useDataStore();
const runtime = useRuntimeStore();
const { checkForUpdate } = useUpdater();

const updateDialogOpen = ref(false);

onMounted(async () => {
  try {
    await data.fetchAll();
    await runtime.startListening();
    await runtime.refresh();
  } catch (e) {
    toast.fromError(e);
  }

  // 启动后静默检查更新（失败不打扰用户，比如 dev 模式或离线）
  try {
    const update = await checkForUpdate();
    if (update) updateDialogOpen.value = true;
  } catch {
    /* 静默忽略 */
  }
});
</script>

<template>
  <div class="flex h-screen w-screen overflow-hidden bg-background">
    <AppSidebar />
    <div class="flex min-w-0 flex-1 flex-col">
      <AppTopbar />
      <main class="flex-1 overflow-y-auto p-6">
        <RouterView />
      </main>
    </div>
    <Toaster />
    <UpdateDialog v-model:open="updateDialogOpen" />
  </div>
</template>
