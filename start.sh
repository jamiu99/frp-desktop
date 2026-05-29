#!/usr/bin/env bash
# frp_desktop 启动 / 停止 / 重启脚本
#
# 用法：
#   ./start.sh start       # 启动桌面 dev 环境（tauri dev，自动管理 vite）
#   ./start.sh stop        # 关闭 tmux session 和所有相关进程
#   ./start.sh restart     # 等价于 stop && start
#   ./start.sh status      # 查看 session 状态
#   ./start.sh attach      # 附着到 tmux session 看实时日志
#   ./start.sh logs        # tail Tauri 日志（包含 vite 输出）
#   ./start.sh web         # 只起前端 vite（浏览器预览 http://localhost:1420，不开桌面窗口）
#
# 设计：
#   - tauri dev 内部已经会拉起 vite（beforeDevCommand），所以只需一个 window
#   - 日志同时落到 logs/ 目录便于离线查看
#   - 不在脚本里做依赖安装；环境问题先手动解决再启

set -euo pipefail

PROJECT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
SESSION="frp-desktop"
LOG_DIR="$PROJECT_DIR/logs"
mkdir -p "$LOG_DIR"

# ---- 环境加载（PATH 里要有 node / cargo） ----
load_env() {
  # nvm
  if [[ -s "$HOME/.nvm/nvm.sh" ]]; then
    # shellcheck disable=SC1091
    \. "$HOME/.nvm/nvm.sh"
  fi
  # 兜底：手动塞 node 路径
  if ! command -v node >/dev/null 2>&1; then
    local node_dir
    node_dir="$(ls -d "$HOME"/.nvm/versions/node/* 2>/dev/null | tail -1)"
    [[ -n "$node_dir" ]] && export PATH="$node_dir/bin:$PATH"
  fi
  # cargo
  if [[ -s "$HOME/.cargo/env" ]]; then
    # shellcheck disable=SC1091
    \. "$HOME/.cargo/env"
  fi
}

ensure_tools() {
  load_env
  local missing=()
  command -v node >/dev/null 2>&1 || missing+=("node")
  command -v pnpm >/dev/null 2>&1 || missing+=("pnpm")
  command -v cargo >/dev/null 2>&1 || missing+=("cargo")
  command -v tmux >/dev/null 2>&1 || missing+=("tmux")
  if [[ ${#missing[@]} -gt 0 ]]; then
    echo "[!] 缺少工具: ${missing[*]}" >&2
    return 1
  fi
}

session_exists() {
  tmux has-session -t "$SESSION" 2>/dev/null
}

cmd_status() {
  if session_exists; then
    echo "[OK] tmux session '$SESSION' 在跑："
    tmux list-windows -t "$SESSION" -F "  - #{window_index}: #{window_name} (#{?window_active,active,idle})"
  else
    echo "[--] tmux session '$SESSION' 未运行"
  fi
}

cmd_start() {
  ensure_tools

  if session_exists; then
    echo "[!] '$SESSION' 已经在跑了。用 './start.sh restart' 重启，或 './start.sh attach' 查看。"
    cmd_status
    return 0
  fi

  echo "[*] 启动 tmux session: $SESSION"

  # 共享 env-loader 文件，让 tmux 子 shell 也能找到 node/cargo
  local envfile="$PROJECT_DIR/.start-env.sh"
  cat > "$envfile" <<'EOF'
[ -s "$HOME/.nvm/nvm.sh" ] && . "$HOME/.nvm/nvm.sh"
if ! command -v node >/dev/null 2>&1; then
  node_dir="$(ls -d "$HOME"/.nvm/versions/node/* 2>/dev/null | tail -1)"
  [ -n "$node_dir" ] && export PATH="$node_dir/bin:$PATH"
fi
[ -s "$HOME/.cargo/env" ] && . "$HOME/.cargo/env"
EOF

  # 单 window: tauri（自动起 vite，再编 Rust，再开桌面窗口）
  tmux new-session -d -s "$SESSION" -n tauri -c "$PROJECT_DIR" \
    "bash -lc 'source \"$envfile\"; pnpm tauri dev 2>&1 | tee \"$LOG_DIR/tauri.log\"'"

  echo "[OK] 已启动。"
  echo "    查看日志:    ./start.sh logs"
  echo "    附着到会话:  ./start.sh attach"
  echo "    停止:        ./start.sh stop"
  echo
  echo "提示: 第一次跑 'pnpm tauri dev' 会编译 Rust 依赖（10~20 分钟）。"
  echo "     编译期间日志看 $LOG_DIR/tauri.log。"
  cmd_status
}

cmd_stop() {
  load_env || true

  if session_exists; then
    echo "[*] 关闭 tmux session: $SESSION"
    tmux kill-session -t "$SESSION"
  else
    echo "[--] tmux session '$SESSION' 不存在"
  fi

  # 兜底：杀掉残留 vite / tauri 子进程
  for kw in "vite" "tauri dev" "cargo run --no-default-features"; do
    pkill -f "$kw" 2>/dev/null && echo "  - killed: $kw" || true
  done

  echo "[OK] 已停止。"
}

cmd_restart() {
  cmd_stop
  sleep 1
  cmd_start
}

cmd_attach() {
  ensure_tools
  if ! session_exists; then
    echo "[!] '$SESSION' 没在跑。先 './start.sh start'"
    exit 1
  fi
  echo "[*] 附着到 $SESSION  （Ctrl+b 然后按 d 可以脱离，不会停止进程）"
  tmux attach -t "$SESSION"
}

cmd_logs() {
  local f="$LOG_DIR/tauri.log"
  [[ -f "$f" ]] || { echo "[!] $f 不存在，可能还没启动过"; exit 1; }
  tail -f "$f"
}

cmd_web() {
  ensure_tools
  echo "[*] 只起前端 dev server（浏览器访问 http://localhost:1420）"
  echo "    Ctrl+C 停止"
  cd "$PROJECT_DIR"
  exec pnpm dev
}

case "${1:-}" in
  start)   cmd_start ;;
  stop)    cmd_stop ;;
  restart) cmd_restart ;;
  status)  cmd_status ;;
  attach)  cmd_attach ;;
  logs)    cmd_logs ;;
  web)     cmd_web ;;
  *)
    cat <<EOF
frp_desktop 启动管理脚本

用法: $0 <command>

命令:
  start     启动桌面 dev 环境（tauri dev，含 vite）
  stop      停止全部
  restart   重启
  status    查看 tmux session 状态
  attach    附着到 tmux 看实时输出（Ctrl+b 然后 d 脱离）
  logs      tail tauri 日志（vite 输出也在里面）
  web       只起前端 vite，不开桌面窗口（http://localhost:1420，看 UI 最快）

示例:
  $0 start
  $0 logs
  $0 status
EOF
    ;;
esac
