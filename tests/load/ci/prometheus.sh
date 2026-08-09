#!/usr/bin/env bash
# tests/load/ci/prometheus.sh
# 抓取服务端 Prometheus metrics(主服务 /metrics)快照, 归档到 results/metrics/。
# 服务端指标: CPU/内存/连接池(metrics-exporter-prometheus), 业务函数 histogram(metrics 宏)。
#
# 用法: source env.sh 后调用, 或 make prometheus [TAG=<任意标签>]

set -euo pipefail
source "$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/env.sh"

TAG="${TAG:-$(date +%Y%m%d-%H%M%S)}"
mkdir -p "$METRICS_DIR"

out="$METRICS_DIR/prom-$TAG.txt"
if curl -sf --max-time 10 "http://${REMOTE_HOST}:${SERVER_PORT}/metrics" -o "$out"; then
    lines=$(wc -l < "$out" | tr -d ' ')
    echo "[prometheus] 快照已保存: $out ($lines 行)"
else
    echo "[prometheus] 警告: 无法访问 ${REMOTE_HOST}:${SERVER_PORT}/metrics, 请确认 server 已启用 metrics feature"
    exit 1
fi

# 摘要: 列出可用的业务 histogram(metric 宏生成的 *_seconds bucket 系列)
if command -v grep >/dev/null 2>&1; then
    echo "[prometheus] 已采集指标组(histogram):"
    grep -E '_seconds_bucket\{' "$out" | sed 's/^\([a-z0-9_:]*\).*/\1/' | sort -u | head -30 || true
fi
