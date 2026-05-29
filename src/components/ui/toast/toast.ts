import { reactive } from "vue";

export interface ToastItem {
  id: number;
  message: string;
  type: "info" | "success" | "error";
  durationMs: number;
}

const state = reactive({
  items: [] as ToastItem[],
});

let nextId = 1;

function push(message: string, type: ToastItem["type"], durationMs = 3500) {
  const id = nextId++;
  state.items.push({ id, message, type, durationMs });
  setTimeout(() => {
    const idx = state.items.findIndex((i) => i.id === id);
    if (idx >= 0) state.items.splice(idx, 1);
  }, durationMs);
}

export const toast = {
  state,
  info: (m: string) => push(m, "info"),
  success: (m: string) => push(m, "success"),
  error: (m: string) => push(m, "error", 5000),
  /** 把后端抛出的 string / Error 友好显示 */
  fromError(err: unknown) {
    const m = typeof err === "string" ? err : (err as Error)?.message ?? String(err);
    push(m, "error", 5000);
  },
};
