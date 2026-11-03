#!/usr/bin/env bash
# 计算运行时资产(libs + models)的内容 hash。
#
# 这个 hash 是"运行环境"的身份标识, 不是二进制版本:
#   - 只要 thirdparty/{onnxruntime,models} 的任一文件内容变化, hash 就变化;
#   - 二进制发布包的 manifest.yaml 里记录该 hash, 部署时按 hash 安装对应资产;
#   - 资产包发布为 runtime-assets-<hash 前 12 位>.tar.gz, 文件名即内容寻址。
#
# 符号链接按 "link -> 目标字符串" 计入(不跟随), 保证 .so 版本链本身也可复现。
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(dirname "$SCRIPT_DIR")"
cd "$REPO_ROOT"

for dir in thirdparty/onnxruntime thirdparty/models; do
    if [ ! -d "$dir" ]; then
        echo "错误: 缺少目录 $dir" >&2
        exit 1
    fi
done

find thirdparty/onnxruntime thirdparty/models \( -type f -o -type l \) -print0 \
    | LC_ALL=C sort -z \
    | while IFS= read -r -d '' path; do
        if [ -L "$path" ]; then
            printf '%s\tlink\t%s\n' "$path" "$(readlink "$path")"
        else
            printf '%s\tfile\t%s\n' "$path" "$(sha256sum "$path" | cut -d' ' -f1)"
        fi
    done \
    | sha256sum \
    | cut -d' ' -f1
