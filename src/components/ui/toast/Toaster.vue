<script setup lang="ts">
import { CheckCircle2, Info, XCircle } from "@lucide/vue";
import { toast } from "./toast";
import { cn } from "@/lib/utils";

const iconMap = {
  success: CheckCircle2,
  info: Info,
  error: XCircle,
};

const colorMap = {
  success: "border-success/50 text-foreground",
  info: "border-border text-foreground",
  error: "border-destructive/50 text-foreground",
};
</script>

<template>
  <div
    class="pointer-events-none fixed inset-x-0 bottom-4 z-[100] flex flex-col items-center gap-2 px-4"
  >
    <transition-group name="toast" tag="div" class="flex w-full max-w-sm flex-col gap-2">
      <div
        v-for="item in toast.state.items"
        :key="item.id"
        :class="
          cn(
            'pointer-events-auto flex items-start gap-3 rounded-md border bg-card p-3 shadow-lg',
            colorMap[item.type],
          )
        "
      >
        <component
          :is="iconMap[item.type]"
          :class="
            cn(
              'mt-0.5 h-5 w-5 shrink-0',
              item.type === 'success' && 'text-success',
              item.type === 'error' && 'text-destructive',
              item.type === 'info' && 'text-muted-foreground',
            )
          "
        />
        <p class="text-sm leading-snug">{{ item.message }}</p>
      </div>
    </transition-group>
  </div>
</template>

<style scoped>
.toast-enter-active,
.toast-leave-active {
  transition: all 200ms ease;
}
.toast-enter-from {
  opacity: 0;
  transform: translateY(8px);
}
.toast-leave-to {
  opacity: 0;
  transform: translateY(-8px);
}
</style>
