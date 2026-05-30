#!/usr/bin/env bash
#
# calendar-notify 一键安装脚本
#
#   安装:  curl -fsSL https://raw.githubusercontent.com/TSK-io/test_email/main/install.sh | bash
#   卸载:  curl -fsSL https://raw.githubusercontent.com/TSK-io/test_email/main/install.sh | bash -s -- uninstall
#
# 做的事:下载最新 Release 二进制 -> 装到 /usr/local/bin -> 自动抓取 RESEND_API_KEY 写入
#         /etc/calendar-notify/env(600)-> 生成 systemd 服务(以当前用户运行、开机自启、
#         崩溃自动重启)-> 启动。装完什么都不用管。
#
set -euo pipefail

REPO="TSK-io/test_email"
ASSET="calendar-notify-x86_64-linux"
BIN_PATH="/usr/local/bin/calendar-notify"
ENV_DIR="/etc/calendar-notify"
ENV_FILE="$ENV_DIR/env"
UNIT_PATH="/etc/systemd/system/calendar-notify.service"
SERVICE="calendar-notify"

log()  { printf '\033[1;32m==>\033[0m %s\n' "$*"; }
warn() { printf '\033[1;33m[警告]\033[0m %s\n' "$*" >&2; }
die()  { printf '\033[1;31m[错误]\033[0m %s\n' "$*" >&2; exit 1; }

# root 直接执行,否则用 sudo 提权
if [ "$(id -u)" -eq 0 ]; then
  SUDO=""
else
  command -v sudo >/dev/null 2>&1 || die "当前非 root 且没有 sudo,无法安装。请用 root 运行。"
  SUDO="sudo"
fi

# 以哪个用户身份运行服务:优先 sudo 的原始用户,其次当前用户
RUN_USER="${SUDO_USER:-$(id -un)}"
RUN_HOME="$(getent passwd "$RUN_USER" 2>/dev/null | cut -d: -f6 || true)"
[ -n "$RUN_HOME" ] || RUN_HOME="${HOME:-/root}"

# ---------- 卸载 ----------
if [ "${1:-}" = "uninstall" ]; then
  log "卸载 $SERVICE ..."
  $SUDO systemctl disable --now "$SERVICE" 2>/dev/null || true
  $SUDO rm -f "$UNIT_PATH"
  $SUDO systemctl daemon-reload 2>/dev/null || true
  $SUDO rm -f "$BIN_PATH"
  $SUDO rm -rf "$ENV_DIR"
  log "已卸载(你的 data.yaml 事件文件未改动)。"
  exit 0
fi

# ---------- 架构检查 ----------
ARCH="$(uname -m)"
[ "$ARCH" = "x86_64" ] || warn "本机架构是 $ARCH,而二进制是 x86_64;若运行报 'Exec format error',需要重编 aarch64 版本。"

# ---------- 找出 RESEND_API_KEY ----------
find_key() {
  if [ -n "${RESEND_API_KEY:-}" ]; then printf '%s' "$RESEND_API_KEY"; return 0; fi
  local f line v
  for f in "$RUN_HOME/.bashrc" "$RUN_HOME/.bash_profile" "$RUN_HOME/.profile" "$RUN_HOME/.zshrc" /etc/environment; do
    [ -r "$f" ] || continue
    line="$(grep -E '^[[:space:]]*(export[[:space:]]+)?RESEND_API_KEY=' "$f" 2>/dev/null | tail -n1 || true)"
    [ -n "$line" ] || continue
    v="${line#*=}"        # 取 = 之后
    v="${v%% *}"          # 砍掉第一个空格之后(去尾部注释)
    case "$v" in          # 去掉成对引号
      \"*\") v="${v#\"}"; v="${v%\"}" ;;
      \'*\') v="${v#\'}"; v="${v%\'}" ;;
    esac
    if [ -n "$v" ]; then printf '%s' "$v"; return 0; fi
  done
  return 1
}
KEY="$(find_key || true)"
[ -n "$KEY" ] || die "没找到 RESEND_API_KEY。请确认 'echo \$RESEND_API_KEY' 有值,或这样运行:
    RESEND_API_KEY=你的密钥 bash -c \"\$(curl -fsSL https://raw.githubusercontent.com/$REPO/main/install.sh)\""

# ---------- 下载二进制(始终取最新 Release) ----------
URL="https://github.com/$REPO/releases/latest/download/$ASSET"
TMP="$(mktemp)"
trap 'rm -f "$TMP"' EXIT
log "下载二进制:$URL"
if command -v curl >/dev/null 2>&1; then
  curl -fsSL "$URL" -o "$TMP" || die "下载失败,检查网络或 Release 是否存在。"
elif command -v wget >/dev/null 2>&1; then
  wget -qO "$TMP" "$URL" || die "下载失败,检查网络或 Release 是否存在。"
else
  die "需要 curl 或 wget。"
fi
$SUDO install -m 0755 "$TMP" "$BIN_PATH"
log "已安装:$BIN_PATH"

# ---------- 写入密钥文件 ----------
$SUDO mkdir -p "$ENV_DIR"
printf 'RESEND_API_KEY=%s\n' "$KEY" | $SUDO tee "$ENV_FILE" >/dev/null
$SUDO chmod 600 "$ENV_FILE"
log "已写入密钥:$ENV_FILE (600)"

# ---------- 事件文件路径 ----------
CAL_FILE="${CAL_FILE:-$RUN_HOME/dotfiles/docs/data.yaml}"
[ -f "$CAL_FILE" ] || warn "事件文件暂不存在:$CAL_FILE(没关系,程序每轮会重读,文件出现后自动生效)"

# ---------- 生成 systemd 服务 ----------
log "写入 systemd 服务:$UNIT_PATH(运行用户 = $RUN_USER)"
$SUDO tee "$UNIT_PATH" >/dev/null <<UNIT
[Unit]
Description=calendar-notify 日历提醒守护进程
Documentation=https://github.com/$REPO
After=network-online.target
Wants=network-online.target

[Service]
Type=simple
User=$RUN_USER
Environment=CAL_FILE=$CAL_FILE
EnvironmentFile=$ENV_FILE
ExecStart=$BIN_PATH run
Restart=on-failure
RestartSec=10

[Install]
WantedBy=multi-user.target
UNIT

# ---------- 启用并启动 ----------
$SUDO systemctl daemon-reload
$SUDO systemctl enable --now "$SERVICE"

log "完成 ✅ 服务已启动并设为开机自启。"
echo
echo "  查看状态:  systemctl status $SERVICE --no-pager"
echo "  实时日志:  journalctl -u $SERVICE -f"
echo "  列出事件:  calendar-notify list"
echo "  发测试邮件: RESEND_API_KEY=\$(sudo cat $ENV_FILE | cut -d= -f2-) calendar-notify test"
echo "  卸载:       curl -fsSL https://raw.githubusercontent.com/$REPO/main/install.sh | bash -s -- uninstall"
