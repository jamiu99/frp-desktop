export type ProxyRunStatus = "stopped" | "starting" | "running" | "crashed";

export interface ProxyRuntime {
  proxy_id: string;
  status: ProxyRunStatus;
  pid: number | null;
  last_error: string | null;
  log_tail: string[];
}
