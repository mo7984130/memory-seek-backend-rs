#!/bin/bash
set -e

IMAGE_NAME="ubuntu22.04-rust-build-base"
PROJECT_NAME="memory-seek-server"
FEATURES="metrics,auth,user,visual,face-engine,audit,audit-recording,backup"

# 构建基础镜像（如果不存在）
if ! podman image exists $IMAGE_NAME; then
    echo -e "${YELLOW}构建基础镜像 $IMAGE_NAME...${NC}"

    # 尝试拉取基础镜像
    echo -e "${YELLOW}尝试拉取基础镜像...${NC}"
    podman pull docker.1panel.live/ubuntu:22.04 || {
        echo -e "${YELLOW}从1panel拉取失败，尝试官方源...${NC}"
        podman pull ubuntu:22.04
    }

    podman build -t $IMAGE_NAME -f Dockerfile.build .
    echo -e "${GREEN}基础镜像构建完成${NC}"
else
    echo -e "${GREEN}基础镜像已存在: $IMAGE_NAME${NC}"
fi

# 运行构建容器
echo -e "${GREEN}开始构建项目...${NC}"
podman run -it --rm \
  --http-proxy=false \
  --name rust-build-$(date +%s) \
  -v "$(pwd):/app:Z" \
  -v "$HOME/.rustup:/root/.rustup:Z" \
  -v "$HOME/.cargo:/root/.cargo:Z" \
  -v "$HOME/.cargo/registry:/root/.cargo/registry:Z" \
  $IMAGE_NAME \
  bash -c "
    set -e
    export PATH=\"/root/.cargo/bin:\$PATH\"
    cd /app

    echo '=== 环境信息 ==='
    rustc --version
    cargo --version
    echo '=== 开始构建 ==='

    # 构建项目
    cargo build \
      --release \
      -p server \
      --features \"$FEATURES\"

    echo '=== 构建完成 ==='
  "

# 检查构建结果
if [ ! -f "target/release/$PROJECT_NAME" ]; then
    echo -e "${RED}错误: 构建失败，未找到 $PROJECT_NAME${NC}"
    exit 1
fi

echo -e "${GREEN}构建成功！${NC}"

# -------------------------
# 打包
# -------------------------

DIST="target/dist"

rm -rf "$DIST"
mkdir -p "$DIST/libs"

# 可执行文件
cp target/release/memory-seek-server "$DIST/"

# libs
find thirdparty \( -type f -o -type l \) \
  \( -name "*.so" -o -name "*.so.*" \) \
  -exec cp -a {} $DIST/libs/ \;

echo "Build completed:"
tree "$DIST"
