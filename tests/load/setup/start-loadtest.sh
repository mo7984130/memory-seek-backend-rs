#!/bin/bash
set -euo pipefail

# ============================================================
# 压测临时服务启动脚本（服务器端执行）
# 用法: ./start-loadtest.sh <DB_USER> <DB_PASS> <AUTH_USERS> <PHOTO_USERS> <PHOTOS> \
#         <S3_ENDPOINT> <S3_ACCESS_KEY> <S3_SECRET_KEY> <S3_REGION> <S3_BUCKET> <S3_PUBLIC_URL> \
#         <TOKEN_KEY> <TOKEN_SALT> [SERVER_BIN]
# ============================================================

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
DB_USER="$1"
DB_PASS="$2"
AUTH_USERS="${3:-10000}"
PHOTO_USERS="${4:-20}"
PHOTOS="${5:-100000}"
S3_ENDPOINT="$6"
S3_ACCESS_KEY="$7"
S3_SECRET_KEY="$8"
S3_REGION="$9"
S3_BUCKET="${10}"
S3_PUBLIC_URL="${11}"
TOKEN_KEY="${12}"
TOKEN_SALT="${13}"
SERVER_BIN="${14:-./server}"

CONFIG_SRC="${SCRIPT_DIR}/loadtest-config.json"
CONFIG_DST="/tmp/config.loadtest.json"
PID_FILE="/tmp/loadtest-server.pid"
DB_NAME="memory_seek_loadtest"
PORT=7985

echo "=== 压测环境启动 ==="

# 1. 创建压测数据库
echo "📦 创建数据库 ${DB_NAME}..."
PGPASSWORD="${DB_PASS}" psql -h localhost -U "${DB_USER}" -d postgres \
  -c "DROP DATABASE IF EXISTS ${DB_NAME};" \
  -c "CREATE DATABASE ${DB_NAME};"

# 2. 建表（使用 init.sql 作为唯一表结构来源）
echo "📋 建表..."
PGPASSWORD="${DB_PASS}" psql -h localhost -U "${DB_USER}" -d "${DB_NAME}" \
  -f "${SCRIPT_DIR}/../../../docs/sql/init.sql"

# 3. 填充数据
echo "🌱 填充数据 (auth_users=${AUTH_USERS}, photo_users=${PHOTO_USERS}, photos=${PHOTOS})..."
PGPASSWORD="${DB_PASS}" psql -h localhost -U "${DB_USER}" -d "${DB_NAME}" \
  -v auth_users="${AUTH_USERS}" \
  -v photo_users="${PHOTO_USERS}" \
  -v photos="${PHOTOS}" \
  -f "${SCRIPT_DIR}/seed.sql"

# 4. 生成配置文件
echo "⚙️  生成配置..."
sed -e "s|DB_USER|${DB_USER}|g" \
    -e "s|DB_PASS|${DB_PASS}|g" \
    -e "s|S3_ENDPOINT|${S3_ENDPOINT}|g" \
    -e "s|S3_ACCESS_KEY|${S3_ACCESS_KEY}|g" \
    -e "s|S3_SECRET_KEY|${S3_SECRET_KEY}|g" \
    -e "s|S3_REGION|${S3_REGION}|g" \
    -e "s|S3_BUCKET|${S3_BUCKET}|g" \
    -e "s|S3_PUBLIC_URL|${S3_PUBLIC_URL}|g" \
    -e "s|TOKEN_KEY|${TOKEN_KEY}|g" \
    -e "s|TOKEN_SALT|${TOKEN_SALT}|g" \
    "$CONFIG_SRC" > "$CONFIG_DST"

# 5. 启动临时服务
echo "🚀 启动临时服务 (port=${PORT})..."
cd "$(dirname "$SERVER_BIN")"
MEMORY_SEEK_CONFIG_PATH="$CONFIG_DST" "$SERVER_BIN" &
echo $! > "$PID_FILE"
echo "   PID=$(cat "$PID_FILE")"

# 6. 等待服务就绪
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
