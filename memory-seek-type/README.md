# memory-seek-type

前端（Rust + WASM）和后端（Rust + Axum）共享的请求/响应模型和验证逻辑。

## 依赖

- `serde` - 序列化/反序列化
- `validator` - 数据验证
- `chrono` - 时间处理

## 使用方法

### 作为 Git submodule

```bash
git submodule add <repository-url> memory-seek-type
```

### 在 Cargo.toml 中引用

```toml
[dependencies]
memory-seek-type = { path = "../memory-seek-type" }
```

## 模块结构

- `auth` - 认证相关类型
- `user` - 用户相关类型
- `photo` - 照片相关类型
