#!/usr/bin/env bash
set -euo pipefail

# ── 参数解析 ──────────────────────────────────────────
HOST=""
USER=""
SERVER_BIN=""
SSH_KEY=""

while [[ $# -gt 0 ]]; do
    case "$1" in
        --host)      HOST="$2"; shift 2 ;;
        --user)      USER="$2"; shift 2 ;;
        --server-bin) SERVER_BIN="$2"; shift 2 ;;
        --ssh-key)   SSH_KEY="$2"; shift 2 ;;
        *) echo "Unknown option: $1"; exit 1 ;;
    esac
done

if [[ -z "$HOST" || -z "$USER" ]]; then
    echo "Usage: $0 --host <IP> --user <USER> [--server-bin <path>] [--ssh-key <path>]"
    exit 1
fi

REMOTE="${USER}@${HOST}"
REMOTE_DIR="~/loadtest"
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
LOAD_DIR="$(dirname "$SCRIPT_DIR")"

# SSH ControlMaster: 只输一次密码，后续复用连接
SSH_SOCKET="/tmp/loadtest-ssh-%C"
SSH_OPTS="-o StrictHostKeyChecking=no -o ConnectTimeout=5 -o ControlMaster=auto -o ControlPath=$SSH_SOCKET -o ControlPersist=600"
if [[ -n "$SSH_KEY" ]]; then
    SSH_OPTS="$SSH_OPTS -i $SSH_KEY"
fi

ssh_cmd() { ssh $SSH_OPTS "$REMOTE" "$@"; }
scp_cmd() { scp $SSH_OPTS "$@"; }

# 清理 SSH 连接
cleanup() { ssh -O stop -o ControlPath="$SSH_SOCKET" "$REMOTE" 2>/dev/null || true; }
trap cleanup EXIT

echo "=== Deploying to $REMOTE ==="

# ── Step 1: 创建远程目录 ─────────────────────────────
echo "[1/7] Creating remote directory..."
ssh_cmd "mkdir -p $REMOTE_DIR"

# ── Step 2: 上传文件 ─────────────────────────────────
echo "[2/7] Uploading files..."
scp_cmd "$LOAD_DIR/docker-compose.yml" "$REMOTE:$REMOTE_DIR/"
scp_cmd "$LOAD_DIR/config/config.json" "$REMOTE:$REMOTE_DIR/config.json"
scp_cmd "$SCRIPT_DIR/../../../docs/sql/init.sql" "$REMOTE:$REMOTE_DIR/"
scp_cmd "$LOAD_DIR/setup/seed.sql" "$REMOTE:$REMOTE_DIR/"
scp_cmd "$LOAD_DIR/setup/verify.sql" "$REMOTE:$REMOTE_DIR/"
if [[ -n "$SERVER_BIN" ]]; then
    scp_cmd "$SERVER_BIN" "$REMOTE:$REMOTE_DIR/memory-seek-server"
fi

# ── Step 3: 启动基础设施 ─────────────────────────────
echo "[3/7] Starting infrastructure (postgres + redis + minio)..."
ssh_cmd "cd $REMOTE_DIR && docker compose up -d"

# ── Step 4: 等待容器就绪 ─────────────────────────────
echo "[4/7] Waiting for containers to be healthy..."
ssh_cmd "cd $REMOTE_DIR && timeout 90 bash -c '
    while true; do
        healthy=\$(docker inspect --format=\"{{.State.Health.Status}}\" \
            \$(docker compose ps -q) 2>/dev/null | grep -c \"healthy\" || true)
        if [ \"\$healthy\" -ge 3 ]; then break; fi
        sleep 2
    done
'" || {
    echo "Warning: containers may not all be healthy, continuing anyway..."
    ssh_cmd "cd $REMOTE_DIR && docker compose ps"
}

# ── Step 5: 创建数据库并导入测试数据 ──────────────────
echo "[5/7] Creating database and seeding data..."
PGCONTAINER=$(ssh_cmd "docker compose -f $REMOTE_DIR/docker-compose.yml ps -q postgres")
ssh_cmd "docker exec -i $PGCONTAINER \
    psql -U test -d postgres -c \"CREATE DATABASE memory_seek_loadtest;\" 2>/dev/null || true"
echo "  Running init.sql (create tables)..."
ssh_cmd "docker exec -i $PGCONTAINER \
    psql -U test -d memory_seek_loadtest" < "$SCRIPT_DIR/../../../docs/sql/init.sql"
echo "  Running seed.sql (insert test data)..."
ssh_cmd "docker exec -i $PGCONTAINER \
    psql -U test -d memory_seek_loadtest -v auth_users=10000 -v photo_users=20 -v photos=100000" < "$LOAD_DIR/setup/seed.sql"

# 验证数据
echo "  Verifying seed data..."
ssh_cmd "docker exec -i $PGCONTAINER \
    psql -U test -d memory_seek_loadtest" < "$LOAD_DIR/setup/verify.sql"

# ── Step 6: 启动服务器 ───────────────────────────────
if [[ -n "$SERVER_BIN" ]]; then
    echo "[6/7] Starting server..."
    ssh_cmd "cd $REMOTE_DIR && chmod +x memory-seek-server && nohup ./memory-seek-server --config config.json > server.log 2>&1 &"

    echo "[7/7] Waiting for server to be ready..."
    ssh_cmd "timeout 30 bash -c 'until curl -sf http://localhost:3000/health >/dev/null 2>&1; do sleep 1; done'" || {
        echo "Warning: server may not be ready"
    }
else
    echo "[6/7] No server binary provided, skipping server start"
    echo "[7/7] Done (infra only)"
fi

echo "=== Deployment complete ==="
echo "PostgreSQL: $HOST:5433"
echo "Redis:      $HOST:6380"
echo "MinIO:      $HOST:9000"
if [[ -n "$SERVER_BIN" ]]; then
    echo "Server:     $HOST:3000"
fi
