#!/usr/bin/env bash
set -euo pipefail

# ── 参数解析 ──────────────────────────────────────────
HOST=""
USER=""

while [[ $# -gt 0 ]]; do
    case "$1" in
        --host) HOST="$2"; shift 2 ;;
        --user) USER="$2"; shift 2 ;;
        *) echo "Unknown option: $1"; exit 1 ;;
    esac
done

if [[ -z "$HOST" || -z "$USER" ]]; then
    echo "Usage: $0 --host <IP> --user <USER>"
    exit 1
fi

REMOTE="${USER}@${HOST}"
REMOTE_DIR="~/loadtest"

# SSH ControlMaster: 只输一次密码，后续复用连接
SSH_SOCKET="/tmp/loadtest-ssh-%C"
SSH_OPTS="-o StrictHostKeyChecking=no -o ConnectTimeout=5 -o ControlMaster=auto -o ControlPath=$SSH_SOCKET -o ControlPersist=600"

ssh_cmd() { ssh $SSH_OPTS "$REMOTE" "$@"; }

# 清理 SSH 连接
cleanup() { ssh -O stop -o ControlPath="$SSH_SOCKET" "$REMOTE" 2>/dev/null || true; }
trap cleanup EXIT

echo "=== Tearing down on $REMOTE ==="

# ── Step 1: 杀掉 server 进程 ─────────────────────────
echo "[1/3] Stopping server..."
ssh_cmd "pkill -f 'memory-seek-server' 2>/dev/null || true"

# ── Step 2: 停止并删除容器和数据卷 ────────────────────
echo "[2/3] Stopping containers and removing volumes..."
ssh_cmd "cd $REMOTE_DIR && docker compose down -v 2>/dev/null || true"

# ── Step 3: 删除上传的文件 ────────────────────────────
echo "[3/3] Cleaning up remote files..."
ssh_cmd "rm -rf $REMOTE_DIR"

echo "=== Teardown complete ==="
