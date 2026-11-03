#!/usr/bin/env bash
# 递增全局版本(server/Cargo.toml 的 package version)。
#
# 全局版本只有一处: server 的 version。其余子 crate 保留各自版本,
# 不随本脚本变化。修改任何子 crate 时都应在同一 PR 里执行本脚本。
#
# 用法: tools/bump-version.sh <x.y.z>
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(dirname "$SCRIPT_DIR")"
MANIFEST="$REPO_ROOT/server/Cargo.toml"

usage() {
    echo "用法: $(basename "$0") <x.y.z>" >&2
    echo "示例: $(basename "$0") 2.7.0" >&2
    exit 1
}

[[ $# -eq 1 ]] || usage
NEW_VERSION="$1"

# 只接受 X.Y.Z, 允许 semver 的预发布/构建元数据后缀
if ! [[ "$NEW_VERSION" =~ ^[0-9]+\.[0-9]+\.[0-9]+([-+][0-9A-Za-z.-]+)?$ ]]; then
    echo "错误: '$NEW_VERSION' 不是合法的 semver(期望 X.Y.Z)" >&2
    exit 1
fi

# 只读取 [package] 段内的 version, 避免误取依赖里的 version
current_version() {
    awk '
        /^\[/ { if (done) exit; in_pkg = ($0 == "[package]"); next }
        in_pkg && /^version = "/ {
            sub(/^version = "/, "")
            sub(/"$/, "")
            print
            done = 1
            exit
        }
    ' "$MANIFEST"
}

CURRENT_VERSION="$(current_version)"
if [[ -z "$CURRENT_VERSION" ]]; then
    echo "错误: 未在 $MANIFEST 的 [package] 段找到 version" >&2
    exit 1
fi

if [[ "$CURRENT_VERSION" == "$NEW_VERSION" ]]; then
    echo "错误: 版本已是 $NEW_VERSION, 无需修改" >&2
    exit 1
fi

# 只替换 [package] 段内的 version 行
tmp="$(mktemp)"
awk -v new="$NEW_VERSION" '
    /^\[/ { if (!done) in_pkg = ($0 == "[package]") }
    in_pkg && !done && /^version = "/ {
        print "version = \"" new "\""
        done = 1
        next
    }
    { print }
' "$MANIFEST" > "$tmp"
mv "$tmp" "$MANIFEST"

# 刷新 Cargo.lock 中 server 的版本
if command -v cargo >/dev/null 2>&1; then
    cargo metadata --format-version 1 --manifest-path "$REPO_ROOT/Cargo.toml" >/dev/null
else
    echo "警告: 未找到 cargo, Cargo.lock 未刷新, 请手动执行 cargo metadata" >&2
fi

echo "版本: $CURRENT_VERSION -> $NEW_VERSION"
git -C "$REPO_ROOT" --no-pager diff -- server/Cargo.toml Cargo.lock
