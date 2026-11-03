#!/usr/bin/env bash
# 发布用构建。
#
# feature 组合是**发布契约**：release workflow 用它产出线上二进制，CI 用同一份
# 脚本验证"发布配置能编过"，避免出现"clippy/单测过了，但发布构型编不过"的情况
# （例如 `admin/face` 只在发布构型下才会被实例化）。
#
# 因此这里固定使用 --all-features -p server，改这里即同时改变发布与 CI。
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(dirname "$SCRIPT_DIR")"
cd "$REPO_ROOT"

cargo build --release --all-features -p server
