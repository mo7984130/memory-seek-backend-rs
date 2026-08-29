#!/usr/bin/env bash
# tests/load/ci/judge.sh
# 当前归一化结果(results/normalized/) 对比基线(baselines/),
# 按 thresholds.yml 判定每个场景: IMPROVED / UNCHANGED / REGRESSION / NO_BASELINE。
# 输出结构化结果 results/verdict.json + 人类可读对比表。
#
# 用法: source env.sh 后调用, 或 make judge

set -euo pipefail
source "$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/env.sh"

command -v jq >/dev/null 2>&1 || { echo "需要 jq, 请先安装"; exit 1; }

THRESHOLDS="${THRESHOLDS:-$LOAD_DIR/thresholds.yml}"
[ -f "$THRESHOLDS" ] || { echo "缺少阈值配置: $THRESHOLDS"; exit 1; }

# ── 解析阈值 ────────────────────────────────────────
t_http_p50_reg=$(awk '/^http_p50:/{f=1;next} /^[a-z_][a-z0-9_]*:/{f=0} f && /regression_pct/{print $2}' "$THRESHOLDS")
t_http_p50_imp=$(awk '/^http_p50:/{f=1;next} /^[a-z_][a-z0-9_]*:/{f=0} f && /improvement_pct/{print $2}' "$THRESHOLDS")
t_http_p95_reg=$(awk '/^http_p95:/{f=1;next} /^[a-z_][a-z0-9_]*:/{f=0} f && /regression_pct/{print $2}' "$THRESHOLDS")
t_http_p95_imp=$(awk '/^http_p95:/{f=1;next} /^[a-z_][a-z0-9_]*:/{f=0} f && /improvement_pct/{print $2}' "$THRESHOLDS")
t_qps_reg=$(awk '/^qps:/{f=1;next} /^[a-z_][a-z0-9_]*:/{f=0} f && /regression_pct/{print $2}' "$THRESHOLDS")
t_qps_imp=$(awk '/^qps:/{f=1;next} /^[a-z_][a-z0-9_]*:/{f=0} f && /improvement_pct/{print $2}' "$THRESHOLDS")
t_op_p95_reg=$(awk '/^op_p95:/{f=1;next} /^[a-z_][a-z0-9_]*:/{f=0} f && /regression_pct/{print $2}' "$THRESHOLDS")
t_op_p95_imp=$(awk '/^op_p95:/{f=1;next} /^[a-z_][a-z0-9_]*:/{f=0} f && /improvement_pct/{print $2}' "$THRESHOLDS")
t_max_err=$(awk '/^max_error_rate:/{print $2}' "$THRESHOLDS")

: "${t_http_p50_reg:=15}" "${t_http_p50_imp:=10}" "${t_http_p95_reg:=15}" "${t_http_p95_imp:=10}"
: "${t_qps_reg:=15}" "${t_qps_imp:=10}" "${t_op_p95_reg:=20}" "${t_op_p95_imp:=15}" "${t_max_err:=0.05}"

VERDICT_FILE="$RESULTS_DIR/verdict.json"
scenarios=""
for f in "$NORMALIZED_DIR"/*.json; do
    [ -f "$f" ] || continue
    scenarios="$scenarios $(basename "$f" .json)"
done
scenarios=$(echo "$scenarios" | tr ' ' '\n' | sort | sed '/^$/d')

if [ -z "$scenarios" ]; then
    echo "[judge] 未发现 normalized 结果, 请先 make collect"
    exit 1
fi

tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT
: > "$tmp/scenarios.jsonl"

for name in $scenarios; do
    cur="$NORMALIZED_DIR/$name.json"
    base="$BASELINES_DIR/$name.json"

    # 构造该场景指标判定对象(逐指标: baseline/current/change_pct/verdict)
    mets=$(mktemp)
    echo '{}' > "$mets"

    add_metric() { # $1 metric名 $2 baseline $3 current $4 is_qps
        local m=$1 b=$2 c=$3 qps_flag=$4
        if [ -z "$b" ] || [ "$b" = "null" ] || [ -z "$c" ] || [ "$c" = "null" ]; then
            return
        fi
        local base_num cur_num chg
        base_num=$(printf '%s' "$b")
        cur_num=$(printf '%s' "$c")
        if [ "$qps_flag" = "1" ]; then
            chg=$(awk -v b="$base_num" -v c="$cur_num" 'BEGIN{if (b==0) {print 0} else {printf "%.4f", (b-c)/b*100}}')
        else
            chg=$(awk -v b="$base_num" -v c="$cur_num" 'BEGIN{if (b==0) {print 0} else {printf "%.4f", (c-b)/b*100}}')
        fi
        # chg 统一为"恶化百分比"(延迟正值=恶化; qps 已换算为负值=恶化)
        local reg_thresh imp_thresh wrsn=0 impr=0
        reg_thresh=$(case "$m" in
            http_p50) echo "$t_http_p50_reg";;
            http_p95) echo "$t_http_p95_reg";;
            qps) echo "$t_qps_reg";;
            op_*) echo "$t_op_p95_reg";;
        esac)
        imp_thresh=$(case "$m" in
            http_p50) echo "$t_http_p50_imp";;
            http_p95) echo "$t_http_p95_imp";;
            qps) echo "$t_qps_imp";;
            op_*) echo "$t_op_p95_imp";;
        esac)
        if awk -v c="$chg" -v t="$reg_thresh" 'BEGIN{exit !(c > t)}'; then wrsn=1; fi
        if awk -v c="$chg" -v t="$imp_thresh" 'BEGIN{exit !(c < -t)}'; then impr=1; fi
        local v="UNCHANGED"
        if [ "$wrsn" = "1" ]; then v="REGRESSION"; elif [ "$impr" = "1" ]; then v="IMPROVED"; fi
        jq --arg m "$m" --argjson b "$base_num" --argjson c "$cur_num" --argjson chg "$chg" --arg v "$v" \
            '. + {($m): {baseline: $b, current: $c, change_pct: $chg, verdict: $v}}' "$mets" > "$mets.tmp"
        mv "$mets.tmp" "$mets"
    }

    if [ -f "$base" ]; then
        # http 层指标
        add_metric http_p50 "$(jq -r '.http_p50 // empty' "$base")" "$(jq -r '.http_p50 // empty' "$cur")" 0
        add_metric http_p95 "$(jq -r '.http_p95 // empty' "$base")" "$(jq -r '.http_p95 // empty' "$cur")" 0
        add_metric qps "$(jq -r '.qps // empty' "$base")" "$(jq -r '.qps // empty' "$cur")" 1

        # op 级 p95
        if [ "$(jq -r '.ops | length' "$cur" 2>/dev/null || echo 0)" -gt 0 ]; then
            while IFS= read -r op; do
                [ -n "$op" ] || continue
                b95=$(jq -r --arg o "$op" '.ops[$o].p95 // empty' "$base")
                c95=$(jq -r --arg o "$op" '.ops[$o].p95 // empty' "$cur")
                add_metric "op_${op}" "$b95" "$c95" 0
            done < <(jq -r '.ops | keys[]' "$cur")
        fi

        # 错误率(绝对阈值)
        cerr=$(jq -r '.error_rate // 0' "$cur")
        if awk -v e="$cerr" -v t="$t_max_err" 'BEGIN{exit !(e > t)}'; then
            jq --argjson e "$cerr" --argjson t "$t_max_err" \
                '. + {"error_rate_over": {baseline: null, current: $e, change_pct: null, verdict: "REGRESSION", note: ("超过上限 " + ($t|tostring))}}' \
                "$mets" > "$mets.tmp"
            mv "$mets.tmp" "$mets"
        fi

        # 场景整体判定
        has_reg=$(jq '[.. | objects | .verdict?] | map(select(. == "REGRESSION")) | length' "$mets")
        has_imp=$(jq '[.. | objects | .verdict?] | map(select(. == "IMPROVED")) | length' "$mets")
        if [ "$has_reg" -gt 0 ]; then sv="REGRESSION"; elif [ "$has_imp" -gt 0 ]; then sv="IMPROVED"; else sv="UNCHANGED"; fi
    else
        sv="NO_BASELINE"
        jq '. + {"_note": "基线缺失: '"$base"'"}' "$mets" > "$mets.tmp"
        mv "$mets.tmp" "$mets"
    fi

    jq -n \
        --arg name "$name" \
        --arg sv "$sv" \
        --slurpfile m "$mets" \
        --arg base_file "${base:-}" \
        '{scenario: $name, verdict: $sv, baseline_file: $base_file, metrics: $m[0]}' \
        >> "$tmp/scenarios.jsonl"
done

# 汇总总体判定
jq -s '
    def verdicts: map(.verdict);
    {overall: (
        if (verdicts | index("REGRESSION")) then "REGRESSION"
        elif (verdicts | index("IMPROVED")) then "IMPROVED"
        else "UNCHANGED" end),
     scenarios: map({key: .scenario, value: {verdict: .verdict, baseline_file: .baseline_file, metrics: .metrics}}) | from_entries,
     generated_at: (now | strftime("%Y-%m-%dT%H:%M:%SZ"))}
' "$tmp/scenarios.jsonl" > "$VERDICT_FILE"

echo "=== 判定结果 ==="
jq -r '.overall as $o |
    "总体判定: " + $o,
    (["场景","判定"] | @tsv),
    (.scenarios | to_entries[] | [.key, .value.verdict] | @tsv),
    ""' "$VERDICT_FILE"
echo "[judge] 详细结果: $VERDICT_FILE"

if [ "$(jq -r '.overall' "$VERDICT_FILE")" = "REGRESSION" ]; then
    echo "[judge] 检测到性能回归" >&2
    exit 1
fi
