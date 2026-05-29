<script setup lang="ts">
import { reactive, ref, watch } from "vue";
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
import { useDataStore } from "@/stores/data";
import { toast } from "@/components/ui/toast";
import type { FrpsServer, ServerInput } from "@/types/store";

const props = defineProps<{
  open: boolean;
  /** 传入则编辑，否则新建 */
  server?: FrpsServer | null;
}>();
const emit = defineEmits<{
  (e: "update:open", v: boolean): void;
}>();

const data = useDataStore();

const form = reactive<ServerInput>({
  name: "",
  host: "",
  port: 7000,
  token: "",
  dashboard_url: "",
  dashboard_user: "",
  dashboard_pass: "",
});
const submitting = ref(false);

watch(
  () => props.open,
  (open) => {
    if (open) {
      if (props.server) {
        Object.assign(form, {
          name: props.server.name,
          host: props.server.host,
          port: props.server.port,
          token: props.server.token,
          dashboard_url: props.server.dashboard_url ?? "",
          dashboard_user: props.server.dashboard_user ?? "",
          dashboard_pass: props.server.dashboard_pass ?? "",
        });
      } else {
        Object.assign(form, {
          name: "",
          host: "",
          port: 7000,
          token: "",
          dashboard_url: "",
          dashboard_user: "",
          dashboard_pass: "",
        });
      }
    }
  },
);

async function submit() {
  submitting.value = true;
  try {
    const payload: ServerInput = {
      ...form,
      dashboard_url: form.dashboard_url?.trim() || null,
      dashboard_user: form.dashboard_user?.trim() || null,
      dashboard_pass: form.dashboard_pass?.trim() || null,
    };
    if (props.server) {
      await data.updateServer(props.server.id, payload);
      toast.success("服务端已更新");
    } else {
      await data.createServer(payload);
      toast.success("服务端已添加");
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
    <DialogContent class="max-w-xl">
      <DialogHeader>
        <DialogTitle>{{ server ? "编辑 frps 服务端" : "添加 frps 服务端" }}</DialogTitle>
        <DialogDescription>
          配置一个 frps 服务端的连接信息和（可选的）dashboard 登录凭证
        </DialogDescription>
      </DialogHeader>

      <form class="space-y-4" @submit.prevent="submit">
        <div class="space-y-2">
          <Label for="name">名称</Label>
          <Input id="name" v-model="form.name" placeholder="如：home-server" />
          <p class="text-xs text-muted-foreground">
            自己看的备注名，可中可英
          </p>
        </div>

        <div class="grid grid-cols-3 gap-3">
          <div class="col-span-2 space-y-2">
            <Label for="host">Host</Label>
            <Input id="host" v-model="form.host" placeholder="frps 公网地址或域名" />
          </div>
          <div class="space-y-2">
            <Label for="port">Port</Label>
            <Input id="port" v-model.number="form.port" type="number" placeholder="7000" />
          </div>
        </div>

        <div class="space-y-2">
          <Label for="token">Token</Label>
          <Input id="token" v-model="form.token" placeholder="frps token，明文存储" />
        </div>

        <div class="rounded-md border bg-muted/30 p-3 space-y-3">
          <p class="text-xs font-medium text-muted-foreground">
            Dashboard（可选）— 用于在应用内查看 frps 状态和 proxy 列表
          </p>
          <div class="space-y-2">
            <Label for="dashboard_url">Dashboard URL</Label>
            <Input
              id="dashboard_url"
              v-model="form.dashboard_url"
              placeholder="http://frps.example.com:5002"
            />
          </div>
          <div class="grid grid-cols-2 gap-3">
            <div class="space-y-2">
              <Label for="dashboard_user">用户名</Label>
              <Input id="dashboard_user" v-model="form.dashboard_user" />
            </div>
            <div class="space-y-2">
              <Label for="dashboard_pass">密码</Label>
              <Input
                id="dashboard_pass"
                v-model="form.dashboard_pass"
                type="password"
              />
            </div>
          </div>
        </div>

        <DialogFooter>
          <DialogClose as-child>
            <Button type="button" variant="outline" :disabled="submitting">
              取消
            </Button>
          </DialogClose>
          <Button type="submit" :disabled="submitting">
            {{ server ? "保存" : "添加" }}
          </Button>
        </DialogFooter>
      </form>
    </DialogContent>
  </Dialog>
</template>
