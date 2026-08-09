#!/usr/bin/env bash
# tests/load/ci/report.sh
# 根据 verdict.json 生成 Markdown 对比报告 -> results/report.md
# 包含: 总体判定、环境指纹、判定总览、逐场景指标对比、服务端指标快照索引。
#
# 用法: source env.sh 后调用, 或 make report

set -euo pipefail
source "$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/env.sh"

command -v jq >/dev/null 2>&1 || { echo "需要 jq, 请先安装"; exit 1; }

VERDICT="$RESULTS_DIR/verdict.json"
[ -f "$VERDICT" ] || { echo "[report] 缺少 $VERDICT, 请先 make judge"; exit 1; }

REPORT="$RESULTS_DIR/report.md"

HOSTNAME=$(hostname 2>/dev/null || echo n/a)
UNAME=$(uname -srmo 2>/dev/null || echo n/a)
NCPU=$(nproc 2>/dev/null || echo n/a)
# 被测 server 绑定核数(控制变量, 见 SERVER_CPUS)
SERVER_CPUS="${SERVER_CPUS:-n/a}"
CPUMODEL=$(lscpu 2>/dev/null | awk -F: '/Model name/{print $2}' | xargs || echo n/a)
MEM=$(free -h 2>/dev/null | awk '/Mem:/{print $2}' || echo n/a)
TS=$(date -Is 2>/dev/null || date +%Y-%m-%dT%H:%M:%S)

{
    echo "# 压测对比报告"
    echo ""
    echo "- 生成时间: $TS"
    echo "- 总体判定: **$(jq -r '.overall' "$VERDICT")**"
    echo ""
    echo "## 环境指纹"
    echo ""
    echo "| 项目 | 值 |"
    echo "|---|---|"
    echo "| Host | $HOSTNAME |"
    echo "| OS | $UNAME |"
    echo "| CPU 核数 | $NCPU |"
    echo "| 被测 CPU 核数 | $SERVER_CPUS |"
    echo "| CPU 型号 | $CPUMODEL |"
    echo "| 内存 | $MEM |"
    echo "| 被测服务 | http://${REMOTE_HOST}:${SERVER_PORT} |"
    echo "| metrics | http://${REMOTE_HOST}:${METRICS_PORT} |"
    echo "| 基线目录 | $BASELINES_DIR |"
    echo ""
    echo "## 判定总览"
    echo ""
    echo "| 场景 | 判定 |"
    echo "|---|---|"
    jq -r '.scenarios | to_entries[] | "| \(.key) | \(.value.verdict) |"' "$VERDICT"
    echo ""
    echo "## 指标对比"
    echo ""
    for name in $(jq -r '.scenarios | keys[]' "$VERDICT"); do
        echo "### $name"
        echo ""
        echo "| 指标 | 基线 | 当前 | 变化% | 判定 |"
        echo "|---|---|---|---|---|"
        jq -r --arg n "$name" '
            .scenarios[$n].metrics | to_entries[]
            | select(.key != "_note")
            | select(.key != "error_rate_over")
            | [.key,
               ((.value.baseline // "—")|tostring),
               ((.value.current // "—")|tostring),
               (if .value.change_pct != null then ((.value.change_pct|tostring)+"%") else "—" end),
               .value.verdict]
            | @tsv' "$VERDICT" | while IFS=$'\t' read -r k b c chg v; do
            echo "| $k | $b | $c | $chg | $v |"
        done
        note=$(jq -r --arg n "$name" '.scenarios[$n].metrics.error_rate_over.note // empty' "$VERDICT")
        if [ -n "$note" ]; then
            echo ""
            echo "> 错误率超限: $note"
        fi
        base_missing=$(jq -r --arg n "$name" '.scenarios[$n].metrics._note // empty' "$VERDICT")
        if [ -n "$base_missing" ]; then
            echo ""
            echo "> $base_missing"
        fi
        echo ""
    done
    echo "## 服务端指标快照"
    echo ""
    if ls "$METRICS_DIR"/prom-*.txt >/dev/null 2>&1; then
        ls -1 "$METRICS_DIR"/prom-*.txt | sed 's/^/- /'
    else
        echo "- (无快照, 请运行 make prometheus)"
    fi
    echo ""
    echo "## 附录"
    echo ""
    echo "- 归一化结果: \`results/normalized/\`"
    echo "- 原始轮次结果: \`results/run-<n>/\`"
    echo "- 判定明细: \`results/verdict.json\`"
} > "$REPORT"

echo "[report] 报告已生成: $REPORT"
