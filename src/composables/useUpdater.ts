import { ref, shallowRef } from "vue";
import { check, type Update } from "@tauri-apps/plugin-updater";
import { relaunch } from "@tauri-apps/plugin-process";

/**
 * 应用自动更新。
 *
 * 流程：check() 读 GitHub 上的 latest.json → 有新版返回 Update 对象 →
 * downloadAndInstall() 下载（验签）+ 安装 → relaunch() 重启生效。
 *
 * 只在打包后的桌面应用里可用；dev 模式 check() 会抛错（无 updater 配置），
 * 调用方需 try/catch。
 *
 * 注意：Update 是带私有字段的 class，用 shallowRef 持有，避免 Vue 深度
 * 响应式把类型展开破坏。
 */

export type UpdatePhase =
  | "idle"
  | "checking"
  | "available"
  | "uptodate"
  | "downloading"
  | "ready"
  | "error";

const phase = ref<UpdatePhase>("idle");
const errorMessage = ref<string>("");
const downloaded = ref(0);
const total = ref<number | null>(null);
/** 当前可用的更新对象（保留 class 类型） */
const currentUpdate = shallowRef<Update | null>(null);

export function useUpdater() {
  async function checkForUpdate(): Promise<Update | null> {
    phase.value = "checking";
    errorMessage.value = "";
    try {
      const update = await check();
      if (update) {
        currentUpdate.value = update;
        phase.value = "available";
        return update;
      }
      currentUpdate.value = null;
      phase.value = "uptodate";
      return null;
    } catch (e) {
      errorMessage.value =
        typeof e === "string" ? e : (e as Error)?.message ?? String(e);
      phase.value = "error";
      return null;
    }
  }

  async function downloadAndInstall() {
    const update = currentUpdate.value;
    if (!update) return;
    downloaded.value = 0;
    total.value = null;
    phase.value = "downloading";
    try {
      await update.downloadAndInstall((event) => {
        switch (event.event) {
          case "Started":
            total.value = event.data.contentLength ?? null;
            break;
          case "Progress":
            downloaded.value += event.data.chunkLength;
            break;
          case "Finished":
            break;
        }
      });
      phase.value = "ready";
    } catch (e) {
      errorMessage.value =
        typeof e === "string" ? e : (e as Error)?.message ?? String(e);
      phase.value = "error";
      throw e;
    }
  }

  async function restartApp() {
    await relaunch();
  }

  function reset() {
    phase.value = "idle";
    currentUpdate.value = null;
    errorMessage.value = "";
  }

  return {
    phase,
    errorMessage,
    downloaded,
    total,
    currentUpdate,
    checkForUpdate,
    downloadAndInstall,
    restartApp,
    reset,
  };
}
