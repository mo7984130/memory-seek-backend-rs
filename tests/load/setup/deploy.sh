#!/usr/bin/env bash
set -euo pipefail

# ── 参数解析 ──────────────────────────────────────────
HOST=""
USER=""
SERVER_BIN_PATH=""
SSH_KEY=""
SERVER_PORT="${SERVER_PORT:-7985}"
REMOTE_DIR="${REMOTE_DIR:-/tmp/memory-seek-server/loadtest}"

while [[ $# -gt 0 ]]; do
    case "$1" in
        --host)      HOST="$2"; shift 2 ;;
        --user)      USER="$2"; shift 2 ;;
        --server-bin-path) SERVER_BIN_PATH="$2"; shift 2 ;;
        --ssh-key)   SSH_KEY="$2"; shift 2 ;;
        --port)       SERVER_PORT="$2"; shift 2 ;;
        --remote-dir) REMOTE_DIR="$2"; shift 2 ;;
        *) echo "Unknown option: $1"; exit 1 ;;
    esac
done

if [[ -z "$HOST" || -z "$USER" ]]; then
    echo "Usage: $0 --host <IP> --user <USER> [--server-bin-path <path>] [--ssh-key <path>] [--remote-dir <path>]"
    exit 1
fi

REMOTE="${USER}@${HOST}"
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
LOAD_DIR="$(dirname "$SCRIPT_DIR")"

# SSH ControlMaster: 只输一次密码，后续复用连接
SSH_SOCKET="/tmp/loadtest-ssh-%C"
SSH_OPTS="-o StrictHostKeyChecking=no -o ConnectTimeout=5 -o ControlMaster=auto -o ControlPath=$SSH_SOCKET -o ControlPersist=600 -o ServerAliveInterval=30 -o ServerAliveCountMax=10"
if [[ -n "$SSH_KEY" ]]; then
    SSH_OPTS="$SSH_OPTS -i $SSH_KEY"
fi

ssh_cmd() { ssh $SSH_OPTS "$REMOTE" "$@"; }
scp_cmd() { scp $SSH_OPTS "$@"; }

# 清理 SSH 连接
cleanup() { ssh -O stop -o ControlPath="$SSH_SOCKET" "$REMOTE" 2>/dev/null || true; }
trap cleanup EXIT

echo "=== Deploying to $REMOTE ==="

# ── Step 0: 杀掉旧进程 ──────────────────────────────
echo "[0/7] Killing existing server process (if any)..."
ssh_cmd "
    target='$REMOTE_DIR/memory-seek-server'
    pids=''
    for proc_exe in /proc/[0-9]*/exe; do
        pid=\${proc_exe#/proc/}
        pid=\${pid%/exe}
        exe=\$(readlink \"\$proc_exe\" 2>/dev/null) || continue
        exe=\${exe% (deleted)}
        if [ \"\$exe\" = \"\$target\" ]; then
            pids=\"\$pids \$pid\"
        fi
    done
    pids=\$(echo \$pids | xargs)
    if [ -n \"\$pids\" ]; then
        echo \"  Found running server (PID: \$pids), killing...\"
        echo \"\$pids\" | xargs kill -9 2>/dev/null || true
        for i in \$(seq 1 10); do
            if ! echo \"\$pids\" | xargs kill -0 2>/dev/null; then break; fi
            sleep 0.5
        done
    fi
    rm -f '$REMOTE_DIR/memory-seek-server' 2>/dev/null || true
    true
"

# ── Step 1: 创建远程目录 ─────────────────────────────
echo "[1/7] Creating remote directory..."
ssh_cmd "mkdir -p $REMOTE_DIR"

# ── Step 2: 上传文件 ─────────────────────────────────
echo "[2/7] Uploading files..."
scp_cmd "$LOAD_DIR/docker-compose.yml" "$REMOTE:$REMOTE_DIR/"
scp_cmd "$LOAD_DIR/config/config.yaml" "$REMOTE:$REMOTE_DIR/config.yaml"
scp_cmd "$SCRIPT_DIR/../../../docs/sql/init.sql" "$REMOTE:$REMOTE_DIR/"
scp_cmd "$LOAD_DIR/setup/seed.sql" "$REMOTE:$REMOTE_DIR/"
scp_cmd "$LOAD_DIR/setup/verify.sql" "$REMOTE:$REMOTE_DIR/"
if [[ -n "$SERVER_BIN_PATH" ]]; then
    scp_cmd "$SERVER_BIN_PATH" "$REMOTE:$REMOTE_DIR/memory-seek-server"
fi

# ── Step 3: 启动基础设施 ─────────────────────────────
echo "[3/7] Starting infrastructure (postgres + redis + minio)..."
ssh_cmd "cd $REMOTE_DIR && docker compose up -d"

# ── Step 4: 等待容器就绪 ─────────────────────────────
echo "[4/7] Waiting for containers to be healthy..."
if ! ssh_cmd "cd $REMOTE_DIR && timeout 90 bash -c '
    while true; do
        # 分别检查每个容器
        pg_status=\$(docker inspect --format=\"{{.State.Health.Status}}\" \$(docker compose ps -q postgres) 2>/dev/null || echo \"unknown\")
        redis_status=\$(docker inspect --format=\"{{.State.Health.Status}}\" \$(docker compose ps -q redis) 2>/dev/null || echo \"unknown\")
        minio_status=\$(docker inspect --format=\"{{.State.Health.Status}}\" \$(docker compose ps -q minio) 2>/dev/null || echo \"unknown\")

        echo \"Waiting... PG:\$pg_status Redis:\$redis_status MinIO:\$minio_status\"

        if [ \"\$pg_status\" = \"healthy\" ] && [ \"\$redis_status\" = \"healthy\" ] && [ \"\$minio_status\" = \"healthy\" ]; then
            echo \"All containers healthy\"
            break
        fi

        sleep 2
    done
'"; then
    echo "ERROR: Containers did not become healthy within timeout"
    echo "Current container status:"
    ssh_cmd "cd $REMOTE_DIR && docker compose ps"
    echo "Container logs for postgres:"
    ssh_cmd "cd $REMOTE_DIR && docker compose logs postgres --tail 50"
    exit 1
fi

# ── Step 5: 创建数据库并导入测试数据 ──────────────────
echo "[5/7] Creating database and seeding data..."
echo "  创建数据库..."
ssh_cmd "docker compose -f $REMOTE_DIR/docker-compose.yml exec -T postgres \
    psql -U test -d postgres -c \"CREATE DATABASE memory_seek_loadtest;\" 2>/dev/null || true"

echo "  运行 init.sql（创建表结构）..."
cat "$SCRIPT_DIR/../../../docs/sql/init.sql" | ssh_cmd "docker compose -f $REMOTE_DIR/docker-compose.yml exec -T postgres \
    psql -U test -d memory_seek_loadtest"

echo "  运行 seed.sql（插入测试数据）..."
cat "$LOAD_DIR/setup/seed.sql" | ssh_cmd "docker compose -f $REMOTE_DIR/docker-compose.yml exec -T postgres \
    psql -U test -d memory_seek_loadtest -v auth_users=10000 -v photo_users=200 -v photos=100000"

# 验证数据
echo "  Verifying seed data..."
cat "$LOAD_DIR/setup/verify.sql" | ssh_cmd "docker compose -f $REMOTE_DIR/docker-compose.yml exec -T postgres \
    psql -U test -d memory_seek_loadtest"

# ── Step 6: 启动服务器 ───────────────────────────────
if [[ -n "$SERVER_BIN_PATH" ]]; then
    echo "[6/7] Starting server..."
    ssh $SSH_OPTS -f "$REMOTE" "cd $REMOTE_DIR && chmod +x memory-seek-server && nohup ./memory-seek-server < /dev/null > server.log 2>&1"

    echo "[7/7] Waiting for server to be ready (port $SERVER_PORT)..."
    ssh_cmd "timeout 30 bash -c 'until curl -sf http://localhost:${SERVER_PORT}/health >/dev/null 2>&1; do sleep 1; done'" || {
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
if [[ -n "$SERVER_BIN_PATH" ]]; then
    echo "Server:     $HOST:$SERVER_PORT"
fi
