#!/bin/bash
set -euo pipefail

# ============================================================
# 压测环境清理脚本（服务器端执行）
# 用法: ./stop-loadtest.sh <DB_USER> <DB_PASS>
# ============================================================

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
COMPOSE_DIR="${SCRIPT_DIR}/.."
DB_USER="$1"
DB_PASS="$2"
PID_FILE="/tmp/loadtest-server.pid"
CONFIG_DST="/tmp/config.loadtest.json"

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

# 2. 销毁 docker-compose 基础设施（含数据卷）
echo "🐳 销毁 docker-compose 基础设施..."
cd "$COMPOSE_DIR"
DB_USER="$DB_USER" DB_PASS="$DB_PASS" docker compose down -v
echo "✅ 基础设施已销毁"

# 3. 清理配置文件
if [ -f "$CONFIG_DST" ]; then
    rm -f "$CONFIG_DST"
    echo "🧹 配置文件已清理"
fi

echo "✅ 清理完成"
