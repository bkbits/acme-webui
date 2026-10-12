#!/bin/sh
# mock-acme.sh — 模拟 acme.sh 用于本地/CI 测试（不请求真实 CA 与 DNS API）
# 注意: 真实 acme.sh 为 POSIX sh 脚本，本模拟脚本也保持 POSIX sh 兼容
#
# 用法:
#   ACME_WEBUI_ACME_SH=/path/to/mock-acme.sh ./acme-webui ...
#
# 行为:
#   --issue        : 成功（域名含 "fail" 时失败，用于测试失败路径）；
#                    同时提供 --fullchain-file / --key-file 时生成自签证书写入对应路径（模拟真实 acme.sh）
#   --install-cert : 用 openssl 生成自签名证书写入 --fullchain-file / --key-file
#   其余参数(-d/--dns/--server/--force/--keylength/--ecc 等)仅解析不请求网络
set -u

action=""
domains=""
primary=""
fullchain=""
keyfile=""

while [ $# -gt 0 ]; do
  case "$1" in
    --issue) action="issue" ;;
    --install-cert) action="install" ;;
    -d)
      shift
      [ $# -gt 0 ] && domains="$domains $1"
      [ -z "$primary" ] && primary="$1"
      ;;
    --fullchain-file) shift; fullchain="$1" ;;
    --key-file) shift; keyfile="$1" ;;
  esac
  shift
done

[ -z "$primary" ] && primary="mock.example.com"

echo "[mock acme.sh] action=$action primary=$primary domains=$domains"

for d in $domains; do
  case "$d" in
    *fail*)
      echo "[mock acme.sh] 模拟颁发失败: $d" >&2
      exit 1
      ;;
  esac
done

case "$action" in
  issue)
    if [ -n "$fullchain" ] && [ -n "$keyfile" ]; then
      mkdir -p "$(dirname "$fullchain")" "$(dirname "$keyfile")"
      openssl req -x509 -newkey rsa:2048 -nodes -days 90 \
        -keyout "$keyfile" -out "$fullchain" \
        -subj "/CN=$primary" 2>/dev/null
      chmod 600 "$keyfile"
      echo "[mock acme.sh] installed cert -> $fullchain"
      echo "[mock acme.sh] installed key   -> $keyfile"
    else
      echo "[mock acme.sh] Cert success. (mock)"
    fi
    ;;
  install)
    if [ -z "$fullchain" ] || [ -z "$keyfile" ]; then
      echo "[mock acme.sh] missing --fullchain-file/--key-file" >&2
      exit 1
    fi
    mkdir -p "$(dirname "$fullchain")" "$(dirname "$keyfile")"
    openssl req -x509 -newkey rsa:2048 -nodes -days 90 \
      -keyout "$keyfile" -out "$fullchain" \
      -subj "/CN=$primary" 2>/dev/null
    chmod 600 "$keyfile"
    echo "[mock acme.sh] installed cert -> $fullchain"
    echo "[mock acme.sh] installed key   -> $keyfile"
    ;;
  *)
    echo "[mock acme.sh] unknown action, exit 1" >&2
    exit 1
    ;;
esac

exit 0
