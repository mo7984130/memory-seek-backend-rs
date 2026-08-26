#!/usr/bin/env bash
# 对 max-adaptive 的多轮容量结论取中位数。

set -euo pipefail
source "$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/env.sh"

: "${MAX_ADAPTIVE_RUNS_DIR:=$RESULTS_DIR/max-adaptive-run}"
: "${MAX_ADAPTIVE_NORMALIZED_DIR:=$RESULTS_DIR/max-adaptive-normalized}"
command -v jq >/dev/null 2>&1 || { echo "需要 jq"; exit 1; }

mkdir -p "$MAX_ADAPTIVE_NORMALIZED_DIR"
scenarios=$(find "${MAX_ADAPTIVE_RUNS_DIR}"-* -mindepth 2 -maxdepth 2 -name capacity.json -printf '%h\n' 2>/dev/null | xargs -r -n1 basename | sort -u)

if [ -z "$scenarios" ]; then
    echo "[collect-max-adaptive] 未发现容量搜索结果"
    exit 1
fi

for name in $scenarios; do
    files=$(find "${MAX_ADAPTIVE_RUNS_DIR}"-* -path "*/${name}/capacity.json" -type f | sort)
    jq -s '
      def median: sort | .[(length / 2 | floor)];
      {
        rounds: length,
        stable_rps: (map(.stable_rps) | median),
        first_failure_rps: (map(.first_failure_rps // .ceiling_rps) | median),
        reasons: (map(.reason) | unique)
      }' $files > "$MAX_ADAPTIVE_NORMALIZED_DIR/${name}.json"
    echo "[collect-max-adaptive] $name -> $MAX_ADAPTIVE_NORMALIZED_DIR/${name}.json"
done
