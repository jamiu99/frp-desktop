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
import { NativeSelect } from "@/components/ui/native-select";
import { useDataStore } from "@/stores/data";
import { toast } from "@/components/ui/toast";
import type { Proxy, ProxyInput, ProxyType } from "@/types/store";

const props = defineProps<{
  open: boolean;
  proxy?: Proxy | null;
  /** 创建时可预选服务端 */
  defaultServerId?: string;
}>();
const emit = defineEmits<{
  (e: "update:open", v: boolean): void;
}>();

const data = useDataStore();
const submitting = ref(false);

interface FormState {
  server_id: string;
  name: string;
  description: string;
  proxy_type: ProxyType;
  local_ip: string;
  local_port: number;
  remote_port: number | null;
  custom_domains_text: string;
}

const form = reactive<FormState>({
  server_id: "",
  name: "",
  description: "",
  proxy_type: "tcp",
  local_ip: "127.0.0.1",
  local_port: 22,
  remote_port: null,
  custom_domains_text: "",
});

watch(
  () => props.open,
  (open) => {
    if (!open) return;
    if (props.proxy) {
      Object.assign(form, {
        server_id: props.proxy.server_id,
        name: props.proxy.name,
        description: props.proxy.description,
        proxy_type: props.proxy.proxy_type,
        local_ip: props.proxy.local_ip,
        local_port: props.proxy.local_port,
        remote_port: props.proxy.remote_port,
        custom_domains_text: props.proxy.custom_domains.join(", "),
      });
    } else {
      Object.assign(form, {
        server_id: props.defaultServerId ?? data.servers[0]?.id ?? "",
        name: "",
        description: "",
        proxy_type: "tcp",
        local_ip: "127.0.0.1",
        local_port: 22,
        remote_port: null,
        custom_domains_text: "",
      });
    }
  },
);

const nameValid = computed(
  () => /^[A-Za-z0-9_][A-Za-z0-9_.-]{0,63}$/.test(form.name),
);

const nameConflict = computed(() => {
  if (!form.name) return false;
  return data.proxies.some(
    (p) =>
      p.id !== props.proxy?.id &&
      p.server_id === form.server_id &&
      p.name === form.name,
  );
});

const needsRemotePort = computed(
  () => form.proxy_type === "tcp" || form.proxy_type === "udp",
);
const needsCustomDomains = computed(
  () => form.proxy_type === "http" || form.proxy_type === "https",
);

const formValid = computed(() => {
  if (!form.server_id) return false;
  if (!nameValid.value || nameConflict.value) return false;
  if (!form.local_ip || !form.local_port) return false;
  if (needsRemotePort.value && !form.remote_port) return false;
  if (needsCustomDomains.value && !form.custom_domains_text.trim()) return false;
  return true;
});

async function submit() {
  if (!formValid.value) return;
  submitting.value = true;
  try {
    const payload: ProxyInput = {
      server_id: form.server_id,
      name: form.name,
      description: form.description,
      proxy_type: form.proxy_type,
      local_ip: form.local_ip,
      local_port: form.local_port,
      remote_port: needsRemotePort.value ? form.remote_port : null,
      custom_domains: needsCustomDomains.value
        ? form.custom_domains_text
            .split(",")
            .map((s) => s.trim())
            .filter(Boolean)
        : [],
    };
    if (props.proxy) {
      await data.updateProxy(props.proxy.id, payload);
      toast.success("Proxy 已更新");
    } else {
      await data.createProxy(payload);
      toast.success("Proxy 已创建");
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
    <DialogContent class="max-w-2xl">
      <DialogHeader>
        <DialogTitle>{{ proxy ? "编辑 Proxy" : "新建 Proxy" }}</DialogTitle>
        <DialogDescription>
          填写 frpc 配置信息。同一 frps 服务端下的 name 不可重复。
        </DialogDescription>
      </DialogHeader>

      <form class="space-y-4" @submit.prevent="submit">
        <div class="grid grid-cols-2 gap-3">
          <div class="space-y-2">
            <Label>frps 服务端</Label>
            <NativeSelect v-model="form.server_id">
              <option v-if="data.servers.length === 0" value="">
                先到「frps 服务端」页添加一个
              </option>
              <option v-for="s in data.servers" :key="s.id" :value="s.id">
                {{ s.name }} ({{ s.host }})
              </option>
            </NativeSelect>
          </div>
          <div class="space-y-2">
            <Label>Name</Label>
            <Input v-model="form.name" placeholder="如 nas-ssh、blog-web" />
          </div>
        </div>

        <p
          v-if="form.name"
          class="text-xs"
          :class="
            !nameValid || nameConflict
              ? 'text-destructive'
              : 'text-muted-foreground'
          "
        >
          <span v-if="!nameValid">name 只能字母/数字/下划线/点/连字符，最多 64 字符</span>
          <span v-else-if="nameConflict">同 frps 下已存在同名 proxy：{{ form.name }}</span>
          <span v-else>同一 frps 下不可重名</span>
        </p>

        <div class="space-y-2">
          <Label for="desc">描述（可选）</Label>
          <Textarea
            id="desc"
            v-model="form.description"
            placeholder="可写可不写"
            rows="2"
          />
        </div>

        <div class="rounded-md border bg-muted/30 p-3 space-y-3">
          <div class="grid grid-cols-3 gap-3">
            <div class="space-y-2">
              <Label>类型</Label>
              <NativeSelect v-model="form.proxy_type">
                <option value="tcp">tcp</option>
                <option value="udp">udp</option>
                <option value="http">http</option>
                <option value="https">https</option>
                <option value="stcp">stcp</option>
              </NativeSelect>
            </div>
            <div class="space-y-2 col-span-2">
              <Label>本地 IP</Label>
              <Input v-model="form.local_ip" placeholder="127.0.0.1" />
            </div>
          </div>

          <div class="grid grid-cols-2 gap-3">
            <div class="space-y-2">
              <Label>本地端口</Label>
              <Input v-model.number="form.local_port" type="number" />
            </div>
            <div v-if="needsRemotePort" class="space-y-2">
              <Label>远端端口</Label>
              <Input v-model.number="form.remote_port" type="number" />
            </div>
          </div>

          <div v-if="needsCustomDomains" class="space-y-2">
            <Label>Custom Domains</Label>
            <Input
              v-model="form.custom_domains_text"
              placeholder="逗号分隔，如：app.example.com, alt.example.com"
            />
          </div>
        </div>

        <DialogFooter>
          <DialogClose as-child>
            <Button type="button" variant="outline" :disabled="submitting">取消</Button>
          </DialogClose>
          <Button type="submit" :disabled="submitting || !formValid">
            {{ proxy ? "保存" : "创建" }}
          </Button>
        </DialogFooter>
      </form>
    </DialogContent>
  </Dialog>
</template>
