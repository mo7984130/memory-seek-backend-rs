#!/usr/bin/env bash
# 组装部署产物。两种模式共用同一份复制逻辑, 避免本地与 CI 漂移。
#
#   --mode=release  CI 发布用: 只产二进制 + manifest.yaml(声明依赖的运行时资产)。
#                   体积约为二进制的十分之一, 每次发版都发。
#   --mode=dist     本地/测试用: 产 target/dist 布局(binary + libs + models),
#                   tests/run/docker-compose.yml 直接挂载该目录。
#
# 运行时资产(libs + models)不随二进制发布: 它们由 .github/workflows/runtime-assets.yml
# 依据 tools/runtime-assets-hash.sh 的内容 hash 单独发布, 变更频率极低。
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(dirname "$SCRIPT_DIR")"
BINARY_NAME="memory-seek-server"

mode="dist"
binary=""
out=""
version=""

usage() {
    cat <<'EOF'
用法: build/package-dist.sh --binary <path> [--mode dist|release] [--out <dir>] [--version <x.y.z>]

  --binary   已编译的 memory-seek-server 可执行文件
  --mode     dist(默认): binary + libs + models; release: binary + manifest.yaml
  --out      输出目录, 默认 target/dist
  --version  仅 release 模式需要, 取自 server/Cargo.toml
EOF
}

# 同时支持 --mode=release 与 --mode release 两种写法
args=()
for arg in "$@"; do
    case "$arg" in
        --*=*) args+=("${arg%%=*}" "${arg#*=}") ;;
        *) args+=("$arg") ;;
    esac
done
set -- "${args[@]}"

while [ $# -gt 0 ]; do
    case "$1" in
        --mode) mode="${2:?--mode 需要参数}"; shift 2 ;;
        --binary) binary="${2:?--binary 需要参数}"; shift 2 ;;
        --out) out="${2:?--out 需要参数}"; shift 2 ;;
        --version) version="${2:?--version 需要参数}"; shift 2 ;;
        -h|--help) usage; exit 0 ;;
        *) echo "错误: 未知参数 $1" >&2; usage >&2; exit 1 ;;
    esac
done

if [ -z "$binary" ] || [ ! -f "$binary" ]; then
    echo "错误: --binary 指向的可执行文件不存在: ${binary:-<未提供>}" >&2
    exit 1
fi

case "$mode" in
    dist|release) ;;
    *) echo "错误: --mode 只能是 dist 或 release" >&2; exit 1 ;;
esac

if [ "$mode" = "release" ] && [ -z "$version" ]; then
    echo "错误: release 模式必须提供 --version" >&2
    exit 1
fi

out="${out:-$REPO_ROOT/target/dist}"
rm -rf "$out"
mkdir -p "$out"
install -m 0755 "$binary" "$out/$BINARY_NAME"

if [ "$mode" = "dist" ]; then
    # 动态库: 保留符号链接(.so -> .so.1 -> .so.1.x.y), 二进制 RUNPATH=$ORIGIN/libs
    mkdir -p "$out/libs"
    find "$REPO_ROOT/thirdparty" \( -type f -o -type l \) \
        \( -name "*.so" -o -name "*.so.*" \) \
        -exec cp -a {} "$out/libs/" \;
    mkdir -p "$out/models"
    cp -a "$REPO_ROOT/thirdparty/models/." "$out/models/"
    echo "dist 产物: $out (binary + libs + models)"
    exit 0
fi

assets_hash="$("$REPO_ROOT/tools/runtime-assets-hash.sh")"
cat > "$out/manifest.yaml" <<EOF
# 由 build/package-dist.sh 生成, 请勿手工修改。
# 声明本二进制依赖的运行时资产(libs + models); 部署时按 id 安装对应资产包,
# 资产内容未变则不会重新下载。
version: "$version"
runtime_assets:
  id: "${assets_hash:0:12}"
  source_sha256: "$assets_hash"
EOF

echo "release 产物: $out (binary + manifest.yaml)"
echo "  依赖运行时资产: ${assets_hash:0:12}"
