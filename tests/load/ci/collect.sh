#!/usr/bin/env bash
# tests/load/ci/collect.sh
# 多轮结果归一化: 对 results/run-<n>/ 下同名 summary 的每个指标取中位数,
# 输出到 results/normalized/<scenario>.json, 供 judge 与 report 消费。
#
# 依赖: jq
# 用法: source env.sh 后调用, 或 make collect

set -euo pipefail
source "$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/env.sh"

command -v jq >/dev/null 2>&1 || { echo "需要 jq, 请先安装"; exit 1; }

# 支持目录覆盖: 默认跑 target 的 run-*/normalized, 可指定 max 的 max-run-*/max-normalized
: "${COLLECT_RUNS_DIR:=$RUNS_DIR}"
: "${COLLECT_OUT_DIR:=$NORMALIZED_DIR}"
RUNS_DIR="$COLLECT_RUNS_DIR"
NORMALIZED_DIR="$COLLECT_OUT_DIR"

# 单轮 summary 规范化提取为 {http_p50, http_p95, qps, error_rate, ops:{op:{count,errors,p95}}}
# 说明: k6 summary 的中位数键为 "med"(等价 p50)
EXTRACT_JQ='
  . as $r |
  {
    http_p50: ($r.metrics.http_req_duration.values.med // $r.metrics.http_req_duration.values["p(50)"]),
    http_p95: $r.metrics.http_req_duration.values["p(95)"],
    qps: $r.metrics.http_reqs.values.rate,
    error_rate: $r.metrics.http_req_failed.values.rate,
    ops: (
      [$r.metrics | keys[] | select(test("^op_(.*)_count$"))] as $cks |
      $cks | map(
        . as $c |
        ($c | gsub("^op_"; "") | gsub("_count$"; "")) as $op |
        {key: $op, value: {
          count: $r.metrics[$c].values.count,
          errors: $r.metrics["op_" + $op + "_errors"].values.fails,
          p95: $r.metrics["op_" + $op + "_duration"].values["p(95)"]
        }}
      ) | from_entries
    )
  }
'

# 多轮归一化: 逐字段取中位数
MEDIAN_JQ='
  def median: sort | .[(length/2) | floor];
  {
    _rounds: length,
    http_p50: ([.[].http_p50] | median),
    http_p95: ([.[].http_p95] | median),
    qps: ([.[].qps] | median),
    error_rate: ([.[].error_rate] | median),
    ops: (
      [.[].ops | to_entries[]] as $all |
      ($all | map(.key) | unique) as $names |
      $names | map(
        . as $name |
        ($all | map(select(.key == $name)) | map(.value)) as $vals |
        {key: $name, value: {
          count: ($vals | map(.count) | median),
          errors: ($vals | map(.errors) | median),
          p95: ($vals | map(.p95) | median)
        }}
      ) | from_entries
    )
  }
'

mkdir -p "$NORMALIZED_DIR"
tmpdir=$(mktemp -d)
trap 'rm -rf "$tmpdir"' EXIT

# 收集 scenario 列表(run-<n>/ 下的 *_summary.json 去重)
scenarios=""
for run_dir in "$RUNS_DIR"-*; do
    [ -d "$run_dir" ] || continue
    for f in "$run_dir"/*_summary.json; do
        [ -f "$f" ] || continue
        scenarios="$scenarios $(basename "$f" _summary.json)"
    done
done
scenarios=$(echo "$scenarios" | tr ' ' '\n' | sort -u | sed '/^$/d')

if [ -z "$scenarios" ]; then
    echo "[collect] 未发现任何轮次结果 (${RUNS_DIR}-*). 请先 make run-all."
    exit 1
fi

for name in $scenarios; do
    rounds=0
    : > "$tmpdir/$name.jsonl"
    for run_dir in "$RUNS_DIR"-*; do
        [ -d "$run_dir" ] || continue
        f="$run_dir/${name}_summary.json"
        [ -f "$f" ] || continue
        if jq -c "$EXTRACT_JQ" "$f" >> "$tmpdir/$name.jsonl" 2>/dev/null; then
            rounds=$((rounds + 1))
        fi
    done

    if [ "$rounds" -eq 0 ]; then
        echo "[collect] $name: 无有效轮次, 跳过"
        continue
    fi

    jq -s "$MEDIAN_JQ" "$tmpdir/$name.jsonl" > "$NORMALIZED_DIR/$name.json"
    echo "[collect] $name -> $NORMALIZED_DIR/$name.json (rounds=$rounds)"
done

echo "[collect] 完成, 归一化结果位于 $NORMALIZED_DIR"
