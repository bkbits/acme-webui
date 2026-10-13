#!/bin/sh
# install.sh — 一键安装 acme-webui (Linux / macOS, amd64 / arm64)
#
# 用法:
#   sh install.sh
#   curl -fsSL https://raw.githubusercontent.com/bkbits/acme-webui/main/scripts/install.sh | sudo sh
#
# 安装结果:
#   二进制  /opt/acme-webui/acme-webui
#   数据    /opt/acme-webui/data/acme-webui.db (服务首次启动自动创建)
#   systemd /etc/systemd/system/acme-webui.service
#
# 环境变量覆盖: ACME_WEBUI_HOST (默认 0.0.0.0), ACME_WEBUI_PORT (默认 6980)
set -eu

REPO="bkbits/acme-webui"
INSTALL_DIR="/opt/acme-webui"
BIN="$INSTALL_DIR/acme-webui"
UNIT="/etc/systemd/system/acme-webui.service"
LISTEN_ADDR="${ACME_WEBUI_HOST:-0.0.0.0}"
LISTEN_PORT="${ACME_WEBUI_PORT:-6980}"

# ---------- 0) 权限与依赖 ----------
if [ "$(id -u)" -ne 0 ]; then
  if [ -f "$0" ] && command -v sudo >/dev/null 2>&1; then
    echo "非 root 运行, 通过 sudo 提权重试"
    exec sudo "$0" "$@"
  fi
  echo "安装需要 root 权限: 以 root 运行, 或 curl ... | sudo sh" >&2
  exit 1
fi

PY=python3
command -v python3 >/dev/null 2>&1 || PY=python
command -v "$PY" >/dev/null 2>&1 || { echo "缺少 python3/python, 无法解析 JSON" >&2; exit 1; }
command -v curl >/dev/null 2>&1 || { echo "缺少 curl" >&2; exit 1; }

# ---------- 1) 查询最新 release tag ----------
echo "==> [1/4] 查询最新 release"
API="https://api.github.com/repos/$REPO/releases/latest"
RESP=$(curl -fsS -H "Accept: application/vnd.github+json" "$API") || {
  echo "查询 latest release 失败（仓库可能还没有 release）" >&2
  exit 1
}
TAG=$(printf '%s' "$RESP" | "$PY" -c 'import sys,json;print(json.load(sys.stdin)["tag_name"])')
echo "    最新 release: $TAG"

# ---------- 2) 检测操作系统与架构 ----------
echo "==> [2/4] 检测操作系统与架构"
OS=$(uname -s | tr '[:upper:]' '[:lower:]')
OS_VERSION=$(uname -r)
if [ -r /etc/os-release ]; then
  OS_VERSION=$(. /etc/os-release && echo "${PRETTY_NAME:-$OS_VERSION}")
fi
case "$(uname -m)" in
  x86_64|amd64) ARCH=amd64 ;;
  aarch64|arm64) ARCH=arm64 ;;
  *) echo "不支持的 CPU 架构: $(uname -m)" >&2; exit 1 ;;
esac
case "$OS" in
  linux)  ASSET="acme-webui-linux-$ARCH" ;;
  darwin) ASSET="acme-webui-mac-$ARCH" ;;
  *) echo "不支持的操作系统: $OS（仅支持 linux / mac）" >&2; exit 1 ;;
esac
echo "    系统: $OS_VERSION"
echo "    平台: $OS/$ARCH, 对应资产: $ASSET"

# 校验该资产在此 release 中真实存在
printf '%s' "$RESP" | "$PY" -c '
import sys, json
d = json.load(sys.stdin)
asset = sys.argv[1]
names = [a["name"] for a in d["assets"]]
if asset not in names:
    print("release 中不存在资产 %s, 当前可用:" % asset, file=sys.stderr)
    for n in names:
        print("  - %s" % n, file=sys.stderr)
    sys.exit(1)
' "$ASSET" || exit 1

# ---------- 3) 下载并安装 ----------
echo "==> [3/4] 下载并安装"
TMP=$(mktemp)
trap 'rm -f "$TMP"' EXIT
curl -fL --progress-bar -o "$TMP" \
  "https://github.com/$REPO/releases/download/$TAG/$ASSET"
mkdir -p "$INSTALL_DIR"
install -m 755 "$TMP" "$BIN"
echo "    已安装: $BIN"

# ---------- 4) 写入 systemd unit ----------
echo "==> [4/4] 写入 systemd unit"
cat > "$UNIT" <<EOF
[Unit]
Description=ACME WebUI
After=network.target

[Service]
WorkingDirectory=$INSTALL_DIR
ExecStart=$BIN -h $LISTEN_ADDR -p $LISTEN_PORT
Restart=always
User=root

[Install]
WantedBy=multi-user.target
EOF
echo "    已写入: $UNIT"

if [ -d /run/systemd/system ]; then
  systemctl daemon-reload
  systemctl enable acme-webui
  echo "    已设置开机自启"
else
  echo "    提示: 当前环境未运行 systemd（如 WSL/容器）, 可用时执行:"
  echo "      systemctl daemon-reload && systemctl enable --now acme-webui"
fi

echo ""
echo "安装完成"
echo "  启动:  systemctl start acme-webui"
echo "  状态:  systemctl status acme-webui"
echo "  数据:  $INSTALL_DIR/data/acme-webui.db"
echo "  默认账号 admin / admin123（首次启动创建, 请及时修改密码）"
