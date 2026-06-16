#!/bin/bash
set -euo pipefail

# ============================================================
# 压测环境完整启动脚本（服务器端执行）
# 用法: ./start-loadtest.sh <DB_USER> <DB_PASS> <AUTH_USERS> <PHOTO_USERS> <PHOTOS> [SERVER_BIN]
#
# 前置条件: docker compose 文件和本脚本在同一目录下
# ============================================================

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
DB_USER="$1"
DB_PASS="$2"
AUTH_USERS="${3:-10000}"
PHOTO_USERS="${4:-20}"
PHOTOS="${5:-100000}"
SERVER_BIN="${6:-./server}"

COMPOSE_DIR="${SCRIPT_DIR}/.."
CONFIG_TEMPLATE="${SCRIPT_DIR}/loadtest-config.json"
CONFIG_DST="/tmp/config.loadtest.json"
PID_FILE="/tmp/loadtest-server.pid"
DB_NAME="memory_seek_loadtest"
PORT=7985

echo "=== 压测环境启动 ==="

# 1. 启动基础设施 (PG + Redis + MinIO)
echo "🐳 启动 docker-compose 基础设施..."
cd "$COMPOSE_DIR"
DB_USER="$DB_USER" DB_PASS="$DB_PASS" docker compose up -d

# 2. 等待所有服务健康
echo "⏳ 等待服务就绪..."
for svc in postgres redis minio; do
    echo "   等待 ${svc}..."
    for i in $(seq 1 30); do
        if docker compose ps --format json "$svc" 2>/dev/null | grep -q '"healthy"'; then
            echo "   ✅ ${svc} 就绪"
            break
        fi
        if [ "$i" -eq 30 ]; then
            echo "   ❌ ${svc} 启动超时"
            docker compose logs "$svc"
            exit 1
        fi
        sleep 1
    done
done

# 3. 创建压测数据库（compose 已创建默认 DB，这里创建压测专用 DB）
echo "📦 创建数据库 ${DB_NAME}..."
PGPASSWORD="${DB_PASS}" psql -h localhost -p 5433 -U "${DB_USER}" -d postgres \
  -c "DROP DATABASE IF EXISTS ${DB_NAME};" \
  -c "CREATE DATABASE ${DB_NAME};"

# 4. 建表
echo "📋 建表..."
PGPASSWORD="${DB_PASS}" psql -h localhost -p 5433 -U "${DB_USER}" -d "${DB_NAME}" \
  -f "${SCRIPT_DIR}/../init.sql"

# 5. 填充数据
echo "🌱 填充数据 (auth_users=${AUTH_USERS}, photo_users=${PHOTO_USERS}, photos=${PHOTOS})..."
PGPASSWORD="${DB_PASS}" psql -h localhost -p 5433 -U "${DB_USER}" -d "${DB_NAME}" \
  -v auth_users="${AUTH_USERS}" \
  -v photo_users="${PHOTO_USERS}" \
  -v photos="${PHOTOS}" \
  -f "${SCRIPT_DIR}/seed.sql"

# 6. 生成配置文件（只替换 DB 凭据，其他值已在模板中硬编码）
echo "⚙️  生成配置..."
sed -e "s|DB_USER|${DB_USER}|g" \
    -e "s|DB_PASS|${DB_PASS}|g" \
    "$CONFIG_TEMPLATE" > "$CONFIG_DST"

# 7. 启动临时服务
echo "🚀 启动临时服务 (port=${PORT})..."
cd "$(dirname "$SERVER_BIN")"
MEMORY_SEEK_CONFIG_PATH="$CONFIG_DST" "$SERVER_BIN" &
echo $! > "$PID_FILE"
echo "   PID=$(cat "$PID_FILE")"

# 8. 等待服务就绪
echo "⏳ 等待服务就绪..."
for i in $(seq 1 30); do
    if curl -sf "http://localhost:${PORT}/login" -o /dev/null 2>&1; then
        echo "✅ 临时服务就绪"
        exit 0
    fi
    sleep 1
done
echo "❌ 服务启动超时 (30s)"
exit 1
