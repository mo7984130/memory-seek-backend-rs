#!/usr/bin/env bash
# SLO 驱动的容量搜索：仅在上一档健康时提高固定到达率。
# 用法: max_adaptive.sh <name> <script> <start-rps> <ceiling-rps> <max-vus> <output-dir>

set -euo pipefail
source "$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/env.sh"

name=$1
script=$2
start_rps=$3
ceiling_rps=$4
max_vus=$5
output_dir=$6

: "${MAX_ADAPTIVE_WARMUP:=10s}"
: "${MAX_ADAPTIVE_DURATION:=30s}"
: "${MAX_ADAPTIVE_GROWTH_FACTOR:=1.5}"
: "${MAX_ADAPTIVE_MAX_STEPS:=8}"
: "${MAX_ADAPTIVE_P95_MS:=500}"
: "${MAX_ADAPTIVE_ERROR_RATE:=0.01}"
: "${MAX_ADAPTIVE_DROPPED_ITERATIONS:=0}"
: "${PHOTOS_PER_USER:=20}"
: "${PHOTO_COUNT:=40000}"
: "${FACES:=40000}"
: "${PERSONS:=2000}"

command -v jq >/dev/null 2>&1 || { echo "需要 jq"; exit 1; }
mkdir -p "$output_dir"

run_k6() {
    local rate=$1 duration=$2 summary=$3
    k6 run -q \
        -e BASE_URL="http://${REMOTE_HOST}:${SERVER_PORT}" \
        -e TARGET_RPS="$rate" \
        -e DURATION="$duration" \
        -e PRE_ALLOCATED_VUS="$max_vus" \
        -e MAX_VUS="$max_vus" \
        -e AUTH_USERS="$AUTH_USERS" \
        -e PHOTO_USERS="$PHOTO_USERS" \
        -e PHOTOS_PER_USER="$PHOTOS_PER_USER" \
        -e PHOTO_COUNT="$PHOTO_COUNT" \
        -e FACES="$FACES" \
        -e PERSONS="$PERSONS" \
        -e SUMMARY_EXPORT="$summary" \
        "$script"
}

# 预热不进入容量判定，避免连接、缓存和 JIT 冷启动污染首档结果。
if [ "$MAX_ADAPTIVE_WARMUP" != "0s" ] && [ "$MAX_ADAPTIVE_WARMUP" != "0" ]; then
    echo "=== $name warmup: ${start_rps} rps ==="
    run_k6 "$start_rps" "$MAX_ADAPTIVE_WARMUP" "$output_dir/warmup_summary.json"
fi

rate=$start_rps
step=1
stable_rps=0
first_failure_rps=null
reason="ceiling_reached"
: > "$output_dir/steps.jsonl"

while [ "$rate" -le "$ceiling_rps" ] && [ "$step" -le "$MAX_ADAPTIVE_MAX_STEPS" ]; do
    summary="$output_dir/step-$(printf '%02d' "$step")-${rate}rps_summary.json"
    echo "=== $name capacity step $step: ${rate} rps ==="
    run_k6 "$rate" "$MAX_ADAPTIVE_DURATION" "$summary"

    metrics=$(jq -c '
      {
        rate: (.metrics.http_reqs.values.rate // 0),
        p95: (.metrics.http_req_duration.values["p(95)"] // 0),
        error_rate: (.metrics.http_req_failed.values.rate // 0),
        dropped_iterations: (.metrics.dropped_iterations.values.count // 0)
      }' "$summary")
    printf '%s\n' "$(jq -cn --argjson target_rps "$rate" --argjson metrics "$metrics" '{target_rps: $target_rps} + $metrics')" >> "$output_dir/steps.jsonl"

    failed=""
    p95=$(jq -r '.p95' <<<"$metrics")
    error_rate=$(jq -r '.error_rate' <<<"$metrics")
    dropped=$(jq -r '.dropped_iterations' <<<"$metrics")
    if awk -v value="$p95" -v limit="$MAX_ADAPTIVE_P95_MS" 'BEGIN { exit !(value > limit) }'; then
        failed="p95_exceeded"
    elif awk -v value="$error_rate" -v limit="$MAX_ADAPTIVE_ERROR_RATE" 'BEGIN { exit !(value > limit) }'; then
        failed="error_rate_exceeded"
    elif awk -v value="$dropped" -v limit="$MAX_ADAPTIVE_DROPPED_ITERATIONS" 'BEGIN { exit !(value > limit) }'; then
        failed="dropped_iterations_exceeded"
    fi

    if [ -n "$failed" ]; then
        first_failure_rps=$rate
        reason=$failed
        break
    fi

    stable_rps=$rate
    next_rate=$(awk -v rate="$rate" -v factor="$MAX_ADAPTIVE_GROWTH_FACTOR" 'BEGIN { printf "%d", rate * factor + 0.999999 }')
    if [ "$next_rate" -le "$rate" ]; then next_rate=$((rate + 1)); fi
    rate=$next_rate
    step=$((step + 1))
done

if [ "$first_failure_rps" = "null" ] && [ "$step" -gt "$MAX_ADAPTIVE_MAX_STEPS" ]; then
    reason="max_steps_reached"
fi

jq -s \
    --arg name "$name" \
    --argjson start_rps "$start_rps" \
    --argjson ceiling_rps "$ceiling_rps" \
    --argjson stable_rps "$stable_rps" \
    --argjson first_failure_rps "$first_failure_rps" \
    --arg reason "$reason" \
    '{scenario: $name, start_rps: $start_rps, ceiling_rps: $ceiling_rps, stable_rps: $stable_rps, first_failure_rps: $first_failure_rps, reason: $reason, steps: .}' \
    "$output_dir/steps.jsonl" > "$output_dir/capacity.json"

echo "[max-adaptive] $name stable=${stable_rps} first_failure=${first_failure_rps} reason=${reason}"
