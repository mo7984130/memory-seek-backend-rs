#!/usr/bin/env sh
# 渲染 jsonnet 目录下所有 dashboard 到 docs/dashboards/
# 依赖:go-jsonnet 二进制(https://github.com/google/go-jsonnet/releases)
# 用法:
#   sh generate.sh                  # 渲染全部
#   JSONNET=/path/to/jsonnet sh generate.sh   # 指定二进制
set -eu
cd "$(dirname "$0")"
JSONNET="${JSONNET:-jsonnet}"

for f in *.jsonnet; do
  case "$f" in
    g.libsonnet) continue ;;  # 库入口,非 dashboard
  esac
  out="../${f%.jsonnet}.json"
  "$JSONNET" -J vendor "$f" | jq -S 'del(.panels[].pluginVersion)' > "$out"
  echo "generated: ${out}"
done

echo "done."