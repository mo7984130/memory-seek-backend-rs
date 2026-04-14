# OSS 模块从 aws-sdk-s3 迁移到 rust-s3 的计划

## 概述

将 `libs/oss` 模块从 `aws-sdk-s3` 迁移到 `rust-s3` 库，以简化依赖并提高与各种 S3 兼容服务的兼容性。

## 当前实现分析

### 依赖项
- `aws-config = "1.8.14"`
- `aws-sdk-s3 = "1.124.0"`

### 主要功能
1. **S3Client 结构体**: 封装 S3 客户端和配置
2. **核心方法**:
   - `new()`: 初始化客户端
   - `upload()`: 上传文件
   - `delete()`: 删除文件
   - `get_url()`: 获取公开访问 URL
   - `get_signed_url()`: 获取预签名 URL
   - `get_signed_url_with_params()`: 获取带自定义参数的预签名 URL
   - `download()`: 下载文件（流式）
   - `download_with_process()`: 下载文件并应用图片处理参数

### 使用场景
1. **photo_service.rs**: 上传和删除照片
2. **alioss_generator.rs**: 生成预签名 URL（带图片处理参数）
3. **其他模块**: 通过 AppState 访问 S3Client

## 迁移策略

### 阶段 1: 准备工作

#### 1.1 更新 Cargo.toml
- 移除 `aws-config` 和 `aws-sdk-s3` 依赖
- 添加 `rust-s3` 依赖（建议版本 0.37.1 或更高）
- 选择合适的 features：
  - 默认使用 `tokio-rustls-tls`（与项目现有运行时匹配）
  - 如需同步方法，添加 `blocking` feature

#### 1.2 分析 API 差异

**客户端初始化**:
```rust
// aws-sdk-s3
let credentials = Credentials::new(access_key, secret_key, None, None, "static");
let config = aws_config::defaults(aws_config::BehaviorVersion::latest())
    .region(Region::new(region))
    .credentials_provider(credentials)
    .endpoint_url(endpoint)
    .load().await;
let s3_config_builder = aws_sdk_s3::config::Builder::from(&config)
    .force_path_style(force_path_style).build();
let client = Client::from_conf(s3_config_builder);

// rust-s3
use s3::{Bucket, Region};
use s3::creds::Credentials;

let region = Region::Custom { region, endpoint };
let credentials = Credentials::new(
    Some(access_key),
    Some(secret_key),
    None,
    None,
    None
)?;
let bucket = Bucket::new(bucket_name, region, credentials)?;
if force_path_style {
    bucket = bucket.with_path_style();
}
```

**上传文件**:
```rust
// aws-sdk-s3
self.client.put_object()
    .bucket(&self.bucket)
    .key(key)
    .body(data.into())
    .set_content_type(Some(content_type.into()))
    .send()
    .await?;

// rust-s3
bucket.put_object_with_content_type(key, &data, content_type).await?;
```

**删除文件**:
```rust
// aws-sdk-s3
self.client.delete_object()
    .bucket(&self.bucket)
    .key(key)
    .send()
    .await?;

// rust-s3
bucket.delete_object(key).await?;
```

**预签名 URL**:
```rust
// aws-sdk-s3
let presigning_config = PresigningConfig::expires_in(expires)?;
let builder = self.client.get_object().bucket(&self.bucket).key(key);
let output = builder.presigned(presigning_config).await?;
let url = output.uri().to_string();

// rust-s3
let url = bucket.presign_get(key, expires.as_secs() as u32, None).await?;
```

**带自定义参数的预签名 URL**:
```rust
// aws-sdk-s3 (需要自定义请求修改)
builder.customize()
    .map_request(move |mut req| {
        let uri_str = req.uri().to_string();
        let connector = if uri_str.contains('?') { "&" } else { "?" };
        let new_uri_str = format!("{}{}x-oss-process={}", uri_str, connector, process);
        if let Ok(parsed_uri) = new_uri_str.try_into() {
            *req.uri_mut() = parsed_uri;
        }
        Ok(req)
    })
    .presigned(presigning_config)
    .await?;

// rust-s3 (支持自定义查询参数)
let mut custom_queries = HashMap::new();
custom_queries.insert("x-oss-process".into(), process);
let url = bucket.presign_get(key, expires.as_secs() as u32, Some(custom_queries)).await?;
```

**下载文件**:
```rust
// aws-sdk-s3 (返回 ByteStream)
let output = self.client.get_object()
    .bucket(&self.bucket)
    .key(key)
    .send()
    .await?;
Ok(output.body)

// rust-s3 (返回 ResponseData)
let response_data = bucket.get_object(key).await?;
// response_data.bytes() 返回 Vec<u8>
// 或使用流式方法: bucket.get_object_stream(key).await?
```

### 阶段 2: 代码迁移

#### 2.1 重构 S3Client 结构体

**新的结构体设计**:
```rust
use s3::Bucket;
use std::sync::Arc;

#[derive(Clone)]
pub struct S3Client {
    bucket: Arc<Bucket>,
    public_url: String,
}
```

#### 2.2 实现核心方法

1. **new() 方法**:
   - 使用 `Bucket::new()` 创建 bucket
   - 配置 `path_style` 如果需要
   - 返回包装后的 S3Client

2. **upload() 方法**:
   - 使用 `bucket.put_object_with_content_type()`
   - 保持相同的错误处理

3. **delete() 方法**:
   - 使用 `bucket.delete_object()`
   - 保持相同的错误处理

4. **get_url() 方法**:
   - 保持现有实现（拼接 public_url 和 key）

5. **get_signed_url() 方法**:
   - 使用 `bucket.presign_get()`
   - 转换 Duration 到秒数

6. **get_signed_url_with_params() 方法**:
   - 使用 `bucket.presign_get()` 并传入自定义查询参数
   - 将 process 参数转换为 `x-oss-process` 查询参数

7. **download() 方法**:
   - **重要变更**: rust-s3 不直接返回流，需要适配
   - 方案 A: 返回 `Vec<u8>` 并修改调用方
   - 方案 B: 使用 `get_object_stream()` 返回流
   - 方案 C: 保持接口不变，内部转换为兼容类型

8. **download_with_process() 方法**:
   - 类似 download()，需要处理自定义查询参数
   - 可能需要使用底层 HTTP 客户端自定义请求

#### 2.3 处理流式下载的兼容性

**选项 1: 修改返回类型**
```rust
pub async fn download(&self, key: &str) -> Result<Vec<u8>, AppError> {
    let response_data = self.bucket.get_object(key).await
        .map_internal_err("OSS下载失败")?;
    Ok(response_data.bytes().to_vec())
}
```

**选项 2: 使用流式 API**
```rust
pub async fn download_stream(&self, key: &str) -> Result<impl Stream<Item = Result<Bytes, AppError>>, AppError> {
    let stream = self.bucket.get_object_stream(key).await
        .map_internal_err("OSS下载失败")?;
    // 转换流类型
}
```

**选项 3: 保持兼容性（推荐）**
```rust
// 如果项目使用 tokio-util 的 StreamReader，可以转换
use tokio_util::io::ReaderStream;

pub async fn download(&self, key: &str) -> Result<ByteStream, AppError> {
    let response_data = self.bucket.get_object(key).await
        .map_internal_err("OSS下载失败")?;
    // 将 Vec<u8> 转换为 ByteStream
    Ok(ByteStream::from(response_data.bytes().to_vec()))
}
```

### 阶段 3: 测试和验证

#### 3.1 单元测试
- 为每个方法编写单元测试
- 测试正常流程和错误处理
- 测试预签名 URL 的生成

#### 3.2 集成测试
- 测试实际上传、下载、删除功能
- 测试预签名 URL 的有效性
- 测试图片处理参数

#### 3.3 回归测试
- 运行现有的所有测试
- 确保照片上传功能正常
- 确保图片 URL 生成正常

### 阶段 4: 优化和清理

#### 4.1 性能优化
- 评估是否需要连接池
- 考虑是否需要重试机制（rust-s3 默认重试一次）
- 优化错误处理

#### 4.2 文档更新
- 更新 README 或相关文档
- 添加迁移说明

#### 4.3 依赖清理
- 确认没有遗留的 aws-sdk 依赖
- 更新 Cargo.lock

## 详细实施步骤

### 步骤 1: 更新依赖
文件: `libs/oss/Cargo.toml`
- 移除 `aws-config` 和 `aws-sdk-s3`
- 添加 `rust-s3 = "0.37"` 及合适的 features
- 可能需要添加 `http` crate 用于 HeaderMap

### 步骤 2: 重构 lib.rs
文件: `libs/oss/src/lib.rs`

1. 更新导入:
   ```rust
   use s3::{Bucket, Region};
   use s3::creds::Credentials;
   use std::sync::Arc;
   use std::collections::HashMap;
   ```

2. 修改 S3Client 结构体:
   ```rust
   #[derive(Clone)]
   pub struct S3Client {
       bucket: Arc<Bucket>,
       public_url: String,
   }
   ```

3. 重写 `new()` 方法:
   - 创建 Region::Custom
   - 创建 Credentials
   - 创建 Bucket 并配置 path_style

4. 重写 `upload()` 方法:
   - 使用 `put_object_with_content_type`

5. 重写 `delete()` 方法:
   - 使用 `delete_object`

6. 重写 `get_signed_url()` 方法:
   - 使用 `presign_get`

7. 重写 `get_signed_url_with_params()` 方法:
   - 使用 `presign_get` 并传入自定义查询参数

8. 重写 `download()` 方法:
   - 使用 `get_object` 或 `get_object_stream`
   - 处理返回类型兼容性

9. 重写 `download_with_process()` 方法:
   - 使用自定义查询参数

### 步骤 3: 处理 ByteStream 兼容性

如果当前代码依赖 `aws_sdk_s3::primitives::ByteStream`:

1. 检查所有使用 `download()` 的地方
2. 评估是否需要保持 ByteStream 类型
3. 如果需要，创建适配器或使用替代类型（如 `bytes::Bytes`）

### 步骤 4: 更新错误处理

rust-s3 使用 `S3Error`，需要:
1. 保持现有的 `AppError` 错误类型
2. 实现 `S3Error` 到 `AppError` 的转换
3. 保持错误消息的一致性

### 步骤 5: 测试验证

1. 运行 `cargo check` 检查编译错误
2. 运行 `cargo test` 运行所有测试
3. 手动测试关键功能:
   - 照片上传
   - 照片删除
   - 图片 URL 生成
   - 图片下载

### 步骤 6: 构建和部署

1. 运行 `cargo build --release`
2. 检查二进制文件大小变化
3. 部署到测试环境验证

## 潜在风险和缓解措施

### 风险 1: API 不兼容
- **影响**: 某些功能可能无法直接映射
- **缓解**: 仔细对比 API，必要时编写适配层

### 风险 2: 性能差异
- **影响**: 新库的性能特征可能不同
- **缓解**: 进行性能测试，必要时优化配置

### 风险 3: 错误处理差异
- **影响**: 错误类型和消息可能不同
- **缓解**: 统一错误处理，保持用户友好的错误消息

### 风险 4: 阿里云 OSS 特定功能
- **影响**: 图片处理参数等特定功能可能需要特殊处理
- **缓解**: 验证自定义查询参数是否正常工作

## 回滚计划

如果迁移失败:
1. 保留原始代码的备份
2. 恢复 Cargo.toml 中的依赖
3. 恢复 libs/oss/src/lib.rs 的原始代码
4. 重新构建和部署

## 时间估算

- 准备和分析: 1 小时
- 代码迁移: 2-3 小时
- 测试验证: 1-2 小时
- 优化和清理: 1 小时
- **总计**: 5-7 小时

## 成功标准

1. ✅ 所有现有功能正常工作
2. ✅ 所有测试通过
3. ✅ 编译无警告
4. ✅ 性能无明显下降
5. ✅ 依赖项简化
6. ✅ 代码可维护性提高
