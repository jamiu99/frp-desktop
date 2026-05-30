export type ProxyRunStatus = "stopped" | "starting" | "running" | "crashed";

export const STATUS_LABEL: Record<ProxyRunStatus, string> = {
  stopped: "已停止",
  starting: "启动中",
  running: "运行中",
  crashed: "已崩溃",
};

/** 后端按 server_id 维度运行 frpc 进程 */
export interface ServerRuntime {
  server_id: string;
  status: ProxyRunStatus;
  pid: number | null;
  last_error: string | null;
  log_tail: string[];
}
