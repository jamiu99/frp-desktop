<script setup lang="ts">
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

defineProps<{
  open: boolean;
  title: string;
  description?: string;
  confirmText?: string;
  destructive?: boolean;
  loading?: boolean;
}>();
const emit = defineEmits<{
  (e: "update:open", v: boolean): void;
  (e: "confirm"): void;
}>();
</script>

<template>
  <Dialog :open="open" @update:open="emit('update:open', $event)">
    <DialogContent class="max-w-md">
      <DialogHeader>
        <DialogTitle>{{ title }}</DialogTitle>
        <DialogDescription v-if="description">{{ description }}</DialogDescription>
      </DialogHeader>
      <DialogFooter>
        <DialogClose as-child>
          <Button type="button" variant="outline" :disabled="loading">取消</Button>
        </DialogClose>
        <Button
          type="button"
          :variant="destructive ? 'destructive' : 'default'"
          :disabled="loading"
          @click="emit('confirm')"
        >
          {{ confirmText ?? "确认" }}
        </Button>
      </DialogFooter>
    </DialogContent>
  </Dialog>
</template>
