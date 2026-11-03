#!/bin/bash
set -e

# 构建运行镜像(内置 ffmpeg)
#
# 只负责运行底座,不编译应用。应用二进制由 build.sh 生成并打包到
# target/dist,compose 仍从该目录挂载;本镜像提供底座 + ffmpeg + 系统运行库。

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

GREEN=$'\033[0;32m'
NC=$'\033[0m'

RUNTIME_IMAGE="memory-seek-server-test:local"

# 锁定 BtbN autobuild tag + sha256, 保证可复现; 需要加速时可换镜像站
FFMPEG_URL="https://github.com/BtbN/FFmpeg-Builds/releases/download/autobuild-2026-10-21-13-06/ffmpeg-n9.0.2-31-gc89d6ee1ca-linux64-gpl-9.0.tar.xz"
FFMPEG_SHA256="2212009529e7a18809c0a23700e8045364c17853c4390cf8c051e6b65dd97bde"

echo -e "${GREEN}构建运行镜像 $RUNTIME_IMAGE...${NC}"
# 使用宿主网络: 构建容器内可访问宿主 127.0.0.1 上的代理
podman build \
  --network=host \
  -t "$RUNTIME_IMAGE" \
  -f "$SCRIPT_DIR/Dockerfile.runtime" \
  --build-arg "FFMPEG_URL=$FFMPEG_URL" \
  --build-arg "FFMPEG_SHA256=$FFMPEG_SHA256" \
  "$SCRIPT_DIR"

echo -e "${GREEN}运行镜像构建完成: $RUNTIME_IMAGE${NC}"
