<script setup lang="ts">
import { ref, watch } from "vue";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { getVersion } from "@tauri-apps/api/app";
import { useDataStore } from "@/stores/data";
import { runtimeApi } from "@/api/runtime";
import { toast } from "@/components/ui/toast";
import { CheckCircle2, XCircle, RefreshCw } from "@lucide/vue";
import {
  enable as enableAutostart,
  disable as disableAutostart,
  isEnabled as isAutostartEnabled,
} from "@tauri-apps/plugin-autostart";
import { useUpdater } from "@/composables/useUpdater";
import UpdateDialog from "@/components/UpdateDialog.vue";

const data = useDataStore();

// 当前版本号
const appVersion = ref("");
getVersion().then((v) => (appVersion.value = v)).catch(() => {});

// 检查更新
const { checkForUpdate } = useUpdater();
const checkingUpdate = ref(false);
const updateDialogOpen = ref(false);
async function checkUpdate() {
  checkingUpdate.value = true;
  try {
    const update = await checkForUpdate();
    if (update) {
      updateDialogOpen.value = true;
    } else {
      toast.success("已是最新版本");
    }
  } catch (e) {
    toast.fromError(e);
  } finally {
    checkingUpdate.value = false;
  }
}

const frpcPath = ref<string>("");
const closeToTray = ref(false);
const autostart = ref(false);
const showFrpcConsole = ref(false);

watch(
  () => data.settings,
  (s) => {
    frpcPath.value = s.frpc_path ?? "";
    closeToTray.value = s.close_to_tray;
    autostart.value = s.autostart;
    showFrpcConsole.value = s.show_frpc_console;
  },
  { immediate: true, deep: true },
);

async function save() {
  try {
    await data.updateSettings({
      frpc_path: frpcPath.value.trim() || null,
      close_to_tray: closeToTray.value,
      autostart: autostart.value,
      show_frpc_console: showFrpcConsole.value,
    });
    // 同步操作系统层面的开机自启
    try {
      const cur = await isAutostartEnabled();
      if (autostart.value && !cur) await enableAutostart();
      else if (!autostart.value && cur) await disableAutostart();
    } catch (e) {
      console.warn("autostart sync failed:", e);
    }
    toast.success("设置已保存");
  } catch (e) {
    toast.fromError(e);
  }
}

const checkResult = ref<{ ok: boolean; msg: string } | null>(null);
async function checkFrpc() {
  checkResult.value = null;
  try {
    const v = await runtimeApi.checkFrpc();
    checkResult.value = { ok: true, msg: v };
  } catch (e) {
    checkResult.value = {
      ok: false,
      msg: typeof e === "string" ? e : (e as Error).message ?? String(e),
    };
  }
}
</script>

<template>
  <div class="space-y-6">
    <div>
      <h2 class="text-2xl font-semibold tracking-tight">设置</h2>
      <p class="text-sm text-muted-foreground">应用偏好与运行行为</p>
    </div>

    <Card>
      <CardHeader>
        <CardTitle>frpc 二进制</CardTitle>
        <CardDescription>
          留空则按以下顺序查找：与本应用同目录的 frpc → 系统 PATH 中的 frpc
        </CardDescription>
      </CardHeader>
      <CardContent class="space-y-3">
        <div class="space-y-2">
          <Label for="frpc-path">frpc 路径</Label>
          <Input
            id="frpc-path"
            v-model="frpcPath"
            placeholder="例如 /usr/local/bin/frpc 或 C:\\frp\\frpc.exe（可留空）"
          />
        </div>
        <div class="flex gap-2">
          <Button variant="outline" @click="checkFrpc">检查 frpc</Button>
          <Button @click="save">保存</Button>
        </div>
        <div
          v-if="checkResult"
          class="flex items-start gap-2 rounded-md border p-3 text-sm"
          :class="checkResult.ok ? 'border-success/50' : 'border-destructive/50 text-destructive'"
        >
          <component
            :is="checkResult.ok ? CheckCircle2 : XCircle"
            class="mt-0.5 h-4 w-4"
            :class="checkResult.ok ? 'text-success' : 'text-destructive'"
          />
          <span class="font-mono text-xs">{{ checkResult.msg }}</span>
        </div>
      </CardContent>
    </Card>

    <Card>
      <CardHeader>
        <CardTitle>启动行为</CardTitle>
        <CardDescription>关闭主窗口时与系统启动时的行为</CardDescription>
      </CardHeader>
      <CardContent class="space-y-3">
        <label class="flex items-center gap-2 text-sm">
          <input v-model="closeToTray" type="checkbox" />
          关闭窗口时最小化到系统托盘（不退出 frpc）
        </label>
        <label class="flex items-center gap-2 text-sm">
          <input v-model="autostart" type="checkbox" />
          开机自启
        </label>
        <label class="flex items-center gap-2 text-sm">
          <input v-model="showFrpcConsole" type="checkbox" />
          显示 frpc 控制台窗口（仅 Windows，调试时打开；改动后下次启动 proxy 生效）
        </label>
        <Button @click="save">保存</Button>
      </CardContent>
    </Card>

    <Card>
      <CardHeader>
        <CardTitle>关于 / 更新</CardTitle>
        <CardDescription>当前版本 v{{ appVersion || "…" }}</CardDescription>
      </CardHeader>
      <CardContent class="space-y-3">
        <Button variant="outline" :disabled="checkingUpdate" @click="checkUpdate">
          <RefreshCw :class="['h-4 w-4', checkingUpdate && 'animate-spin']" />
          {{ checkingUpdate ? "检查中…" : "检查更新" }}
        </Button>
        <p class="text-xs text-muted-foreground">
          从 GitHub Releases 检查并下载新版本（自动验签后安装）
        </p>
      </CardContent>
    </Card>

    <UpdateDialog v-model:open="updateDialogOpen" />
  </div>
</template>
