#!/bin/bash
set -euo pipefail

# ============================================================
# 压测临时服务停止 + 清理脚本（服务器端执行）
# 用法: ./stop-loadtest.sh <DB_USER> <DB_PASS>
# ============================================================

DB_USER="$1"
DB_PASS="$2"
PID_FILE="/tmp/loadtest-server.pid"
CONFIG_DST="/tmp/config.loadtest.json"
DB_NAME="memory_seek_loadtest"

echo "=== 压测环境清理 ==="

# 1. 停止临时服务
if [ -f "$PID_FILE" ]; then
    PID=$(cat "$PID_FILE")
    if kill -0 "$PID" 2>/dev/null; then
        kill "$PID"
        echo "🛑 临时服务已停止 (PID=${PID})"
    else
        echo "⚠️  临时服务已不存在 (PID=${PID})"
    fi
    rm -f "$PID_FILE"
else
    echo "⚠️  未找到 PID 文件"
fi

# 2. 删除压测数据库
echo "🗑️  删除数据库 ${DB_NAME}..."
PGPASSWORD="${DB_PASS}" psql -h localhost -U "${DB_USER}" -d postgres \
  -c "DROP DATABASE IF EXISTS ${DB_NAME};" 2>/dev/null || true

# 3. 清理配置文件
if [ -f "$CONFIG_DST" ]; then
    rm -f "$CONFIG_DST"
    echo "🧹 配置文件已清理"
fi

echo "✅ 清理完成"
