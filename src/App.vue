<script setup lang="ts">
import { onMounted } from "vue";
import AppSidebar from "@/components/layout/AppSidebar.vue";
import AppTopbar from "@/components/layout/AppTopbar.vue";
import { Toaster } from "@/components/ui/toast";
import { useDataStore } from "@/stores/data";
import { useRuntimeStore } from "@/stores/runtime";
import { toast } from "@/components/ui/toast";

const data = useDataStore();
const runtime = useRuntimeStore();

onMounted(async () => {
  try {
    await data.fetchAll();
    await runtime.startListening();
    await runtime.refresh();
  } catch (e) {
    toast.fromError(e);
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
  </div>
</template>
