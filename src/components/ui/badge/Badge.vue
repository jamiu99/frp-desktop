<script setup lang="ts">
import { type BadgeVariants, badgeVariants } from "./index";
import { cn } from "@/lib/utils";

const props = defineProps<{
  variant?: BadgeVariants["variant"];
  class?: string;
}>();
</script>

<template>
  <span :class="cn(badgeVariants({ variant }), props.class)">
    <slot />
  </span>
</template>
