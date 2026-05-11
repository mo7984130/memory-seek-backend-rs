# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

Memory Seek Backend - Rust 后端服务，提供用户管理、认证和照片管理功能。采用 domain-driven 设计，通过 Cargo features 控制模块编译。

## Build & Development

```bash
# 构建所有功能模块
cargo build

# 仅构建特定功能
cargo build --features "auth user photo metrics"
cargo build --features "auth user photo face_recognition"

# 运行测试
cargo test

# 运行特定测试
cargo test --test test_name
cargo test --package auth
cargo test --lib  # 仅运行库测试

# 性能测试
cargo bench
```

## Architecture

### Workspace 结构

```
memory-seek-backend-rs/
├── server/          # 主入口，axum Web 服务
├── common/          # 公共模块 (错误处理, 响应格式, 工具)
├── domains/         # 业务域 (auth, user, photo)
├── entities/        # 数据库实体 (sea-orm)
├── libs/            # 工具库 (oss, email, face_engine, img_url_generator)
└── benches/         # 性能测试
```

### Feature Flags

server 模块通过 features 控制编译哪些业务模块：
- `auth` - 认证模块 (auth/domain)
- `user` - 用户模块 (user/domain)
- `photo` - 照片模块 (photo/domain)
- `face_recognition` - 人脸识别 (依赖 photo)
- `metrics` - Prometheus 性能监控

### 技术栈

- **Web框架**: axum 0.8
- **ORM**: sea-orm (PostgreSQL)
- **向量数据库**: pgvector (用于人脸特征存储)
- **缓存**: Redis (deadpool-redis)
- **存储**: 阿里云 OSS (rust-s3)
- **人脸引擎**: ORT (ONNX Runtime)

### 关键路径

- 配置文件: `config.json` 或环境变量 `MEMORY_SEEK_CONFIG_PATH`
- 数据库迁移: `common/src/models/`
- 路由挂载: `server/src/setup/` 下各模块初始化
- 中间件: `server/src/middlewares/` (trace_id, auth)

### 响应格式

所有 API 响应使用统一格式: `common/src/r.rs` 中定义的 `R<T>` 结构体。

## Notes

- 模块间依赖通过 `server/src/setup/` 初始化，各域有独立的 state 和 controller
- `entities` 模块包含数据库表结构定义，修改后需同步到对应域的 models
- 性能测试在 `benches/` 中，使用 criterion 框架
