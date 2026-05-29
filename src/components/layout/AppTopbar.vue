<script setup lang="ts">
import { computed } from "vue";
import { useRoute } from "vue-router";

const route = useRoute();
const title = computed(() => (route.meta?.title as string) ?? "");
</script>

<template>
  <header
    class="flex h-14 shrink-0 items-center justify-between border-b bg-background/70 px-6 backdrop-blur"
  >
    <h1 class="text-base font-semibold tracking-tight">{{ title }}</h1>
    <div class="flex items-center gap-2 text-xs text-muted-foreground">
      <slot name="right" />
    </div>
  </header>
</template>
