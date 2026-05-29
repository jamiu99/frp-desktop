<script setup lang="ts">
import { useVModel } from "@vueuse/core";
import { ChevronDown } from "@lucide/vue";
import { cn } from "@/lib/utils";

interface Props {
  modelValue?: string;
  defaultValue?: string;
  class?: string;
  disabled?: boolean;
}

const props = defineProps<Props>();
const emits = defineEmits<{
  (e: "update:modelValue", payload: string): void;
}>();

const modelValue = useVModel(props, "modelValue", emits, {
  passive: true,
  defaultValue: props.defaultValue,
});
</script>

<template>
  <div class="relative">
    <select
      v-model="modelValue"
      :disabled="disabled"
      :class="
        cn(
          'flex h-9 w-full appearance-none rounded-md border border-input bg-transparent px-3 pr-8 py-1 text-sm shadow-sm transition-colors focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-ring disabled:cursor-not-allowed disabled:opacity-50',
          props.class,
        )
      "
    >
      <slot />
    </select>
    <ChevronDown
      class="pointer-events-none absolute right-2 top-1/2 h-4 w-4 -translate-y-1/2 text-muted-foreground"
    />
  </div>
</template>
