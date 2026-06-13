# 构建阶段
FROM rust:1.87-bookworm AS builder

WORKDIR /app
COPY . .

# 安装依赖
RUN apt-get update && apt-get install -y protobuf-compiler && rm -rf /var/lib/apt/lists/*

# 构建 release 版本
RUN cargo build --release --features "auth,user,photo"

# 运行阶段
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# 从构建阶段复制二进制文件
COPY --from=builder /app/target/release/server /usr/local/bin/

# 创建配置目录
RUN mkdir -p /app/config

# 暴露端口
EXPOSE 8080

# 启动服务
CMD ["server"]
