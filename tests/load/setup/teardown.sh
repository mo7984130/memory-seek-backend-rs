#!/usr/bin/env bash
set -euo pipefail

# ── 参数解析 ──────────────────────────────────────────
HOST=""
USER=""
SSH_KEY=""
REMOTE_DIR="/tmp/memory-seek-server/loadtest"

while [[ $# -gt 0 ]]; do
    case "$1" in
        --host)       HOST="$2"; shift 2 ;;
        --user)       USER="$2"; shift 2 ;;
        --ssh-key)    SSH_KEY="$2"; shift 2 ;;
        --remote-dir) REMOTE_DIR="$2"; shift 2 ;;
        *) echo "Unknown option: $1"; exit 1 ;;
    esac
done

if [[ -z "$HOST" || -z "$USER" ]]; then
    echo "Usage: $0 --host <IP> --user <USER> [--ssh-key <path>] [--remote-dir <path>]"
    exit 1
fi

REMOTE="${USER}@${HOST}"

# SSH ControlMaster: 只输一次密码，后续复用连接
SSH_SOCKET="/tmp/loadtest-ssh-%C"
SSH_OPTS="-o StrictHostKeyChecking=no -o ConnectTimeout=5 -o ControlMaster=auto -o ControlPath=$SSH_SOCKET -o ControlPersist=600"
if [[ -n "$SSH_KEY" ]]; then
    SSH_OPTS="$SSH_OPTS -i $SSH_KEY"
fi

ssh_cmd() { ssh $SSH_OPTS "$REMOTE" "$@"; }

# 清理 SSH 连接
cleanup() { ssh -O stop -o ControlPath="$SSH_SOCKET" "$REMOTE" 2>/dev/null || true; }
trap cleanup EXIT

echo "=== Tearing down on $REMOTE ==="

# ── Step 1: 杀掉 server 进程 ─────────────────────────
echo "[1/3] Stopping server..."
ssh_cmd "
    pids=\$(pgrep -f '$REMOTE_DIR/memory-seek-server' | grep -v \$\$ || true)
    if [ -n \"\$pids\" ]; then
        echo \"  Found running server (PID: \$pids), killing...\"
        echo \"\$pids\" | xargs kill -9 2>/dev/null || true
        for i in \$(seq 1 10); do
            if ! echo \"\$pids\" | xargs kill -0 2>/dev/null; then break; fi
            sleep 0.5
        done
    fi
    true
"

# ── Step 2: 停止并删除容器和数据卷 ────────────────────
echo "[2/3] Stopping containers and removing volumes..."
ssh_cmd "cd $REMOTE_DIR && docker compose down -v 2>/dev/null || true"

# ── Step 3: 删除上传的文件 ────────────────────────────
echo "[3/3] Cleaning up remote files..."
ssh_cmd "rm -rf $REMOTE_DIR"

echo "=== Teardown complete ==="
