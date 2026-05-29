#!/usr/bin/env bash
# 下载并校验 frpc 全平台二进制，按 Tauri sidecar 命名放到本目录。
#
# 用法:
#   ./fetch.sh                # 用脚本里的 FRP_VERSION
#   FRP_VERSION=0.70.0 ./fetch.sh   # 临时覆盖
#
# 依赖: curl, tar, unzip, sha256sum

set -euo pipefail

FRP_VERSION="${FRP_VERSION:-0.69.0}"
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT

URL_BASE="https://github.com/fatedier/frp/releases/download/v${FRP_VERSION}"
CHECKSUMS_URL="${URL_BASE}/frp_sha256_checksums.txt"

# 平台 → Tauri target triple → frpc 文件名
declare -A TARGETS=(
  ["linux_amd64"]="x86_64-unknown-linux-gnu:frpc"
  ["linux_arm64"]="aarch64-unknown-linux-gnu:frpc"
  ["darwin_amd64"]="x86_64-apple-darwin:frpc"
  ["darwin_arm64"]="aarch64-apple-darwin:frpc"
  ["windows_amd64"]="x86_64-pc-windows-msvc:frpc.exe"
  ["windows_arm64"]="aarch64-pc-windows-msvc:frpc.exe"
)

echo "[*] frp v${FRP_VERSION} → $HERE"
echo "[*] 下载 checksums"
curl -fsSL "$CHECKSUMS_URL" -o "$WORK/checksums.txt"

for plat in "${!TARGETS[@]}"; do
  pair="${TARGETS[$plat]}"
  triple="${pair%%:*}"
  binname="${pair##*:}"

  case "$plat" in
    windows_*) ext="zip" ;;
    *)         ext="tar.gz" ;;
  esac

  archive="frp_${FRP_VERSION}_${plat}.${ext}"
  echo "[*] $plat → $archive"
  curl -fsSL "${URL_BASE}/${archive}" -o "$WORK/$archive"

  expected=$(grep " ${archive}\$" "$WORK/checksums.txt" | awk '{print $1}')
  actual=$(sha256sum "$WORK/$archive" | awk '{print $1}')
  if [[ "$expected" != "$actual" ]]; then
    echo "[!] checksum mismatch for $archive" >&2
    echo "    expected $expected" >&2
    echo "    actual   $actual" >&2
    exit 1
  fi
  echo "    sha256 ok"

  case "$ext" in
    tar.gz) tar -C "$WORK" -xzf "$WORK/$archive" ;;
    zip)    unzip -q -o "$WORK/$archive" -d "$WORK" ;;
  esac

  src="$WORK/frp_${FRP_VERSION}_${plat}/${binname}"
  if [[ "$ext" = "zip" ]]; then
    out="$HERE/frpc-${triple}.exe"
  else
    out="$HERE/frpc-${triple}"
  fi
  cp "$src" "$out"
  chmod +x "$out" 2>/dev/null || true
  echo "    -> $(basename "$out") ($(du -h "$out" | cut -f1))"
done

cat > "$HERE/VERSION" <<EOF
frp_version = ${FRP_VERSION}
upstream    = https://github.com/fatedier/frp/releases/tag/v${FRP_VERSION}
checksums   = ${CHECKSUMS_URL}
fetched_at  = $(date -I)
EOF

echo
echo "[OK] 全部 ${#TARGETS[@]} 个 sidecar 二进制已就绪"
ls -lh "$HERE"/frpc-* | awk '{print "    " $9 "  " $5}'
