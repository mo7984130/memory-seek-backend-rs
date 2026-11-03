#!/bin/bash
set -e

# 从仓库根目录执行(与在哪里调用脚本无关)
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(dirname "$SCRIPT_DIR")"
cd "$REPO_ROOT"

RED=$'\033[0;31m'
GREEN=$'\033[0;32m'
YELLOW=$'\033[0;33m'
NC=$'\033[0m'

IMAGE_NAME="ubuntu22.04-rust-build-base"
PROJECT_NAME="memory-seek-server"
FEATURES="metrics,auth,user,visual,face-engine,audit,audit-recording,backup,admin"

# 容器内 cargo 产物目录(容器专用), 与宿主 target/ 完全隔离:
# 1) 两边 glibc 不同(宿主较新), 编译产物(尤其 proc-macro .so)不可互相加载;
# 2) 共享同一 target 时, 宿主一次 cargo check 就会改写 rustc 指纹
#    (target/.rustc_info.json), 导致容器缓存整体失效、全量重编译。
CONTAINER_TARGET_DIR="target/container"

# 构建基础镜像（如果不存在）
if ! podman image exists $IMAGE_NAME; then
    echo -e "${YELLOW}构建基础镜像 $IMAGE_NAME...${NC}"

    # 尝试拉取基础镜像
    echo -e "${YELLOW}尝试拉取基础镜像...${NC}"
    podman pull docker.1panel.live/ubuntu:22.04 || {
        echo -e "${YELLOW}从1panel拉取失败，尝试官方源...${NC}"
        podman pull ubuntu:22.04
    }

    podman build --http-proxy=false -t $IMAGE_NAME -f "$SCRIPT_DIR/Dockerfile.build" "$SCRIPT_DIR"
    echo -e "${GREEN}基础镜像构建完成${NC}"
else
    echo -e "${GREEN}基础镜像已存在: $IMAGE_NAME${NC}"
fi

# 运行构建容器
# 预检: .cargo/config.toml 用 `-fuse-ld=mold`, 镜像缺 mold 时 gcc 只会报
# 误导性的 "cannot find 'ld'", 所以先给出明确提示
if ! podman run --rm $IMAGE_NAME sh -c 'command -v mold >/dev/null'; then
    echo -e "${RED}错误: 基础镜像 $IMAGE_NAME 缺少 mold(或无法启动)${NC}"
    echo -e "${YELLOW}删除旧镜像后重跑本脚本, 会自动重建:${NC}"
    echo "  podman rmi $IMAGE_NAME"
    echo "  $0"
    exit 1
fi

echo -e "${GREEN}开始构建项目...${NC}"
# 容器使用独立 target($CONTAINER_TARGET_DIR), 与宿主 target/ 隔离, 避免缓存互相失效。
podman run -i --rm \
  --http-proxy=false \
  --name rust-build-$(date +%s) \
  -e CARGO_TARGET_DIR="/app/$CONTAINER_TARGET_DIR" \
  -v "$REPO_ROOT:/app:Z" \
  -v "$HOME/.rustup:/root/.rustup:Z" \
  -v "$HOME/.cargo:/root/.cargo:Z" \
  -v "$HOME/.cargo/registry:/root/.cargo/registry:Z" \
  "$IMAGE_NAME" \
  bash -s -- "$FEATURES" <<'CONTAINER_BUILD'
set -e
FEATURES="$1"
export PATH="/root/.cargo/bin:$PATH"
cd /app

echo '=== 环境信息 ==='
rustc --version
cargo --version
mold --version
echo '=== 开始构建 ==='

# 构建项目
cargo build \
  --release \
  -p server \
  --features "$FEATURES"

echo '=== 构建完成 ==='
CONTAINER_BUILD

# 检查构建结果
if [ ! -f "$CONTAINER_TARGET_DIR/release/$PROJECT_NAME" ]; then
    echo -e "${RED}错误: 构建失败，未找到 $PROJECT_NAME${NC}"
    exit 1
fi

echo -e "${GREEN}构建成功！${NC}"

# -------------------------
# 打包
# -------------------------

DIST="target/dist"

# 统一由 package-dist.sh 组装, 与 CI 发布包共用同一份复制逻辑
"$SCRIPT_DIR/package-dist.sh" --mode dist --binary "$CONTAINER_TARGET_DIR/release/$PROJECT_NAME" --out "$DIST"

echo "Build completed:"
tree "$DIST"

echo -e "${GREEN}二进制构建完成: $DIST${NC}"
echo -e "${YELLOW}运行镜像请执行: ./build/runtime-image.sh${NC}"
