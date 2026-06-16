#!/bin/bash
set -euo pipefail

# ============================================================
# 压测环境清理脚本（服务器端执行）
#
# 用法:
#   ./stop-loadtest.sh --db-user test --db-pass test
#
# 必填参数:
#   --db-user   PostgreSQL 用户名
#   --db-pass   PostgreSQL 密码
# ============================================================

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
COMPOSE_DIR="${SCRIPT_DIR}/.."
PID_FILE="/tmp/loadtest-server.pid"
CONFIG_DST="/tmp/config.loadtest.json"

# 默认值
DB_USER=""
DB_PASS=""

# 解析命名参数
while [[ $# -gt 0 ]]; do
    case "$1" in
        --db-user)  DB_USER="$2";  shift 2 ;;
        --db-pass)  DB_PASS="$2";  shift 2 ;;
        *)
            echo "未知参数: $1"
            echo "用法: $0 --db-user USER --db-pass PASS"
            exit 1
            ;;
    esac
done

# 校验必填参数
if [[ -z "$DB_USER" || -z "$DB_PASS" ]]; then
    echo "错误: --db-user 和 --db-pass 为必填参数"
    echo "用法: $0 --db-user USER --db-pass PASS"
    exit 1
fi

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
