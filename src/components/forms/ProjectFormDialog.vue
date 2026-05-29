<script setup lang="ts">
import { computed, reactive, ref, watch } from "vue";
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
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Textarea } from "@/components/ui/textarea";
import { useDataStore } from "@/stores/data";
import { toast } from "@/components/ui/toast";
import type { Project, ProjectInput } from "@/types/store";

const props = defineProps<{
  open: boolean;
  project?: Project | null;
}>();
const emit = defineEmits<{
  (e: "update:open", v: boolean): void;
}>();

const data = useDataStore();
const submitting = ref(false);

const form = reactive<ProjectInput>({
  name: "",
  description: "",
  color: null,
});

watch(
  () => props.open,
  (open) => {
    if (open) {
      if (props.project) {
        Object.assign(form, {
          name: props.project.name,
          description: props.project.description,
          color: props.project.color,
        });
      } else {
        Object.assign(form, { name: "", description: "", color: null });
      }
    }
  },
);

const nameValid = computed(() =>
  /^[a-z0-9]([a-z0-9-]{0,30}[a-z0-9])?$/.test(form.name),
);
const descLen = computed(() => form.description.trim().length);
const descValid = computed(() => descLen.value >= 10);

async function submit() {
  if (!nameValid.value) {
    toast.error("项目名格式不对：小写字母/数字/连字符，2~32 字符");
    return;
  }
  if (!descValid.value) {
    toast.error(`描述至少 10 字（当前 ${descLen.value}）`);
    return;
  }

  submitting.value = true;
  try {
    if (props.project) {
      await data.updateProject(props.project.id, { ...form });
      toast.success("项目已更新");
    } else {
      await data.createProject({ ...form });
      toast.success("项目已创建");
    }
    emit("update:open", false);
  } catch (e) {
    toast.fromError(e);
  } finally {
    submitting.value = false;
  }
}
</script>

<template>
  <Dialog :open="open" @update:open="emit('update:open', $event)">
    <DialogContent class="max-w-lg">
      <DialogHeader>
        <DialogTitle>{{ project ? "编辑项目" : "新建项目" }}</DialogTitle>
        <DialogDescription>
          一个项目对应一组功能相关的 Proxy，所有 Proxy 必须挂在某个项目下
        </DialogDescription>
      </DialogHeader>

      <form class="space-y-4" @submit.prevent="submit">
        <div class="space-y-2">
          <Label for="p-name">项目名（slug）</Label>
          <Input
            id="p-name"
            v-model="form.name"
            placeholder="如：nas、company-vpn、home-iot"
            :disabled="!!project"
          />
          <p class="text-xs"
             :class="form.name && !nameValid ? 'text-destructive' : 'text-muted-foreground'">
            小写字母/数字/连字符，2~32 字符。会用作 proxy name 的前缀。
            <span v-if="project" class="ml-1">编辑时不允许改名（会破坏已有 proxy 的命名一致性）</span>
          </p>
        </div>

        <div class="space-y-2">
          <Label for="p-desc">
            描述
            <span class="ml-2 text-xs"
                  :class="descValid ? 'text-muted-foreground' : 'text-destructive'">
              {{ descLen }} / 10+
            </span>
          </Label>
          <Textarea
            id="p-desc"
            v-model="form.description"
            placeholder="这个项目是干嘛的？写清楚，几个月后回来看不会蒙"
            rows="3"
          />
        </div>

        <DialogFooter>
          <DialogClose as-child>
            <Button type="button" variant="outline" :disabled="submitting">取消</Button>
          </DialogClose>
          <Button type="submit" :disabled="submitting || !nameValid || !descValid">
            {{ project ? "保存" : "创建" }}
          </Button>
        </DialogFooter>
      </form>
    </DialogContent>
  </Dialog>
</template>
