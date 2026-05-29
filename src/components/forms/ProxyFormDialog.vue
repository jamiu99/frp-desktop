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
  /** 创建时可预选项目 */
  defaultProjectId?: string;
  /** 创建时可预选服务端 */
  defaultServerId?: string;
}>();
const emit = defineEmits<{
  (e: "update:open", v: boolean): void;
}>();

const data = useDataStore();
const submitting = ref(false);

const PURPOSE_BLACKLIST = new Set([
  "test", "test1", "test2", "temp", "tmp", "a", "b", "c",
  "1", "2", "xxx", "yyy", "demo", "foo", "bar",
]);

interface FormState {
  project_id: string;
  server_id: string;
  purpose: string;
  description: string;
  proxy_type: ProxyType;
  local_ip: string;
  local_port: number;
  remote_port: number | null;
  custom_domains_text: string;
}

const form = reactive<FormState>({
  project_id: "",
  server_id: "",
  purpose: "",
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
        project_id: props.proxy.project_id,
        server_id: props.proxy.server_id,
        purpose: props.proxy.purpose,
        description: props.proxy.description,
        proxy_type: props.proxy.proxy_type,
        local_ip: props.proxy.local_ip,
        local_port: props.proxy.local_port,
        remote_port: props.proxy.remote_port,
        custom_domains_text: props.proxy.custom_domains.join(", "),
      });
    } else {
      Object.assign(form, {
        project_id: props.defaultProjectId ?? data.projects[0]?.id ?? "",
        server_id: props.defaultServerId ?? data.servers[0]?.id ?? "",
        purpose: "",
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

const project = computed(() =>
  data.projects.find((p) => p.id === form.project_id),
);
const computedName = computed(() =>
  project.value && form.purpose ? `${project.value.name}-${form.purpose}` : "",
);

const purposeValid = computed(() => {
  if (!form.purpose) return false;
  if (!/^[a-z0-9]([a-z0-9-]{0,30}[a-z0-9])?$/.test(form.purpose)) return false;
  if (PURPOSE_BLACKLIST.has(form.purpose)) return false;
  return true;
});
const purposeBlacklisted = computed(() =>
  PURPOSE_BLACKLIST.has(form.purpose),
);

const descLen = computed(() => form.description.trim().length);
const descValid = computed(() => descLen.value >= 10);

const nameConflict = computed(() => {
  if (!computedName.value) return false;
  return data.proxies.some(
    (p) =>
      p.id !== props.proxy?.id &&
      p.server_id === form.server_id &&
      p.name === computedName.value,
  );
});

const needsRemotePort = computed(() =>
  form.proxy_type === "tcp" || form.proxy_type === "udp",
);
const needsCustomDomains = computed(() =>
  form.proxy_type === "http" || form.proxy_type === "https",
);

const formValid = computed(() => {
  if (!form.project_id || !form.server_id) return false;
  if (!purposeValid.value || !descValid.value || nameConflict.value) return false;
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
      project_id: form.project_id,
      server_id: form.server_id,
      purpose: form.purpose,
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
          name 由 <code class="font-mono">项目名-用途</code> 自动拼接，描述至少 10 字
        </DialogDescription>
      </DialogHeader>

      <form class="space-y-4" @submit.prevent="submit">
        <div class="grid grid-cols-2 gap-3">
          <div class="space-y-2">
            <Label>项目</Label>
            <NativeSelect v-model="form.project_id" :disabled="!!proxy">
              <option v-if="data.projects.length === 0" value="">先去创建一个项目</option>
              <option v-for="p in data.projects" :key="p.id" :value="p.id">
                {{ p.name }}
              </option>
            </NativeSelect>
          </div>
          <div class="space-y-2">
            <Label>frps 服务端</Label>
            <NativeSelect v-model="form.server_id">
              <option v-if="data.servers.length === 0" value="">先去添加一个服务端</option>
              <option v-for="s in data.servers" :key="s.id" :value="s.id">
                {{ s.name }} ({{ s.host }})
              </option>
            </NativeSelect>
          </div>
        </div>

        <div class="space-y-2">
          <Label for="purpose">用途（purpose）</Label>
          <Input id="purpose" v-model="form.purpose" placeholder="如：ssh、webui、api" />
          <p class="text-xs"
             :class="
               (form.purpose && !purposeValid) || nameConflict
                 ? 'text-destructive'
                 : 'text-muted-foreground'
             ">
            <span v-if="!form.purpose">小写字母/数字/连字符，2~32 字符</span>
            <span v-else-if="purposeBlacklisted">不允许的占位词："{{ form.purpose }}" — 请写有意义的用途</span>
            <span v-else-if="!purposeValid">格式不对：小写字母/数字/连字符开头结尾</span>
            <span v-else-if="nameConflict">同 frps 下已存在 proxy：{{ computedName }}</span>
            <span v-else>name 将是 <code class="font-mono">{{ computedName }}</code></span>
          </p>
        </div>

        <div class="space-y-2">
          <Label for="desc">
            描述
            <span class="ml-2 text-xs"
                  :class="descValid ? 'text-muted-foreground' : 'text-destructive'">
              {{ descLen }} / 10+
            </span>
          </Label>
          <Textarea
            id="desc"
            v-model="form.description"
            placeholder="这个 proxy 是干嘛的？例：从外网 SSH 到家里 NAS；跑 jellyfin web UI"
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
