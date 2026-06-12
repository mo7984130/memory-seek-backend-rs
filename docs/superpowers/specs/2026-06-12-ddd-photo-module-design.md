# Photo模块DDD重构设计

## 1. 概述

本设计文档描述了将photo模块重构为六边形架构（端口与适配器）的详细方案。重构目标是提高代码可维护性、可扩展性、可测试性和团队协作效率。

## 2. 当前架构分析

### 2.1 现有结构

```
domains/photo/
├── controllers/          # HTTP控制器
├── mappers/             # 数据访问层
├── models/              # 数据模型
├── services/            # 业务逻辑
└── state.rs             # 模块状态
```

### 2.2 主要问题

1. **业务逻辑混乱**：业务逻辑分散在控制器和服务中
2. **数据模型贫血**：数据库实体直接暴露给前端，缺乏领域模型
3. **业务规则不明确**：业务规则分散，难以验证和测试
4. **耦合度高**：模块间依赖复杂，修改一处影响多处

## 3. 六边形架构设计

### 3.1 核心概念

六边形架构（端口与适配器）的核心思想是：
- **领域层**：核心业务逻辑，不依赖任何外部技术
- **端口**：定义领域层与外部交互的接口
- **适配器**：实现端口，连接外部技术（数据库、HTTP、消息队列等）

### 3.2 新目录结构

```
domains/photo/
├── domain/                    # 领域层
│   ├── entities/              # 领域实体
│   │   ├── mod.rs
│   │   ├── photo.rs           # Photo聚合根
│   │   ├── collection.rs      # Collection聚合根
│   │   └── comment.rs         # Comment聚合根
│   ├── value_objects/         # 值对象
│   │   ├── mod.rs
│   │   ├── photo_id.rs        # PhotoId值对象
│   │   ├── file_info.rs       # FileInfo值对象
│   │   └── image_token.rs     # ImageToken值对象
│   ├── events/                # 领域事件
│   │   ├── mod.rs
│   │   ├── photo_events.rs    # 照片相关事件
│   │   └── collection_events.rs # 收藏夹相关事件
│   ├── repositories/          # 仓储接口（端口）
│   │   ├── mod.rs
│   │   ├── photo_repository.rs
│   │   ├── collection_repository.rs
│   │   └── comment_repository.rs
│   ├── services/              # 领域服务
│   │   ├── mod.rs
│   │   ├── photo_domain_service.rs
│   │   └── collection_domain_service.rs
│   └── mod.rs
├── application/               # 应用层
│   ├── commands/              # 命令（写操作）
│   │   ├── mod.rs
│   │   ├── upload_photo.rs
│   │   ├── delete_photo.rs
│   │   └── create_collection.rs
│   ├── queries/               # 查询（读操作）
│   │   ├── mod.rs
│   │   ├── get_photo.rs
│   │   ├── list_photos.rs
│   │   └── get_collection.rs
│   ├── handlers/              # 命令/查询处理器
│   │   ├── mod.rs
│   │   ├── photo_command_handler.rs
│   │   ├── photo_query_handler.rs
│   │   └── collection_command_handler.rs
│   ├── event_handlers/        # 事件处理器
│   │   ├── mod.rs
│   │   └── photo_event_handler.rs
│   └── mod.rs
├── infrastructure/            # 基础设施层
│   ├── persistence/           # 持久化适配器
│   │   ├── mod.rs
│   │   ├── photo_repository_impl.rs
│   │   ├── collection_repository_impl.rs
│   │   └── comment_repository_impl.rs
│   ├── http/                  # HTTP适配器
│   │   ├── mod.rs
│   │   ├── photo_controller.rs
│   │   ├── collection_controller.rs
│   │   └── comment_controller.rs
│   ├── storage/               # 存储适配器
│   │   ├── mod.rs
│   │   └── s3_storage_adapter.rs
│   ├── cache/                 # 缓存适配器
│   │   ├── mod.rs
│   │   └── redis_cache_adapter.rs
│   ├── messaging/             # 消息适配器
│   │   ├── mod.rs
│   │   └── event_publisher.rs
│   └── mod.rs
├── ports/                     # 端口定义
│   ├── mod.rs
│   ├── storage_port.rs        # 存储端口
│   ├── cache_port.rs          # 缓存端口
│   ├── event_port.rs          # 事件端口
│   └── messaging_port.rs      # 消息端口
├── lib.rs                     # 模块入口
├── state.rs                   # 模块状态
└── config.rs                  # 模块配置
```

### 3.3 分层职责

#### 3.3.1 领域层（Domain）

**职责**：
- 包含核心业务逻辑
- 定义聚合根、实体、值对象
- 定义仓储接口（端口）
- 发布领域事件

**特点**：
- 不依赖任何外部技术
- 纯粹的业务逻辑
- 高内聚，低耦合

#### 3.3.2 应用层（Application）

**职责**：
- 协调领域对象完成业务用例
- 处理命令和查询
- 处理领域事件
- 事务管理

**特点**：
- 薄层，不包含业务逻辑
- 协调领域对象
- 处理横切关注点

#### 3.3.3 基础设施层（Infrastructure）

**职责**：
- 实现端口（适配器）
- 连接外部技术
- 处理技术细节

**特点**：
- 实现领域层定义的接口
- 处理数据库、HTTP、缓存等
- 可替换，不影响领域逻辑

## 4. 详细设计

### 4.1 领域实体设计

#### 4.1.1 Photo聚合根

```rust
// domain/entities/photo.rs
use crate::domain::value_objects::{PhotoId, FileInfo, ImageDimensions};
use crate::domain::events::PhotoEvent;

pub struct Photo {
    id: PhotoId,
    user_id: i64,
    name: String,
    file_info: FileInfo,
    dimensions: ImageDimensions,
    md5: String,
    created_at: chrono::DateTime<chrono::Utc>,
    updated_at: chrono::DateTime<chrono::Utc>,
    domain_events: Vec<PhotoEvent>,
}

impl Photo {
    pub fn create(
        user_id: i64,
        name: String,
        file_info: FileInfo,
        dimensions: ImageDimensions,
        md5: String,
    ) -> Self {
        let photo = Self {
            id: PhotoId::new(),
            user_id,
            name,
            file_info,
            dimensions,
            md5,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
            domain_events: vec![],
        };
        
        photo.add_event(PhotoEvent::PhotoCreated {
            photo_id: photo.id.clone(),
            user_id,
            created_at: photo.created_at,
        });
        
        photo
    }
    
    pub fn delete(&mut self) {
        self.add_event(PhotoEvent::PhotoDeleted {
            photo_id: self.id.clone(),
            user_id: self.user_id,
            file_id: self.file_info.file_id.clone(),
        });
    }
    
    fn add_event(&mut self, event: PhotoEvent) {
        self.domain_events.push(event);
    }
    
    pub fn take_events(&mut self) -> Vec<PhotoEvent> {
        std::mem::take(&mut self.domain_events)
    }
}
```

#### 4.1.2 PhotoId值对象

```rust
// domain/value_objects/photo_id.rs
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PhotoId(i64);

impl PhotoId {
    pub fn new() -> Self {
        Self(0) // 由数据库生成
    }
    
    pub fn from(id: i64) -> Self {
        Self(id)
    }
    
    pub fn value(&self) -> i64 {
        self.0
    }
}
```

### 4.2 仓储接口设计

#### 4.2.1 PhotoRepository端口

```rust
// domain/repositories/photo_repository.rs
use crate::domain::entities::Photo;
use crate::domain::value_objects::PhotoId;
use async_trait::async_trait;

#[async_trait]
pub trait PhotoRepository: Send + Sync {
    async fn find_by_id(&self, id: &PhotoId) -> Result<Option<Photo>>;
    async fn find_by_ids(&self, ids: &[PhotoId]) -> Result<Vec<Photo>>;
    async fn save(&self, photo: &Photo) -> Result<()>;
    async fn delete(&self, id: &PhotoId) -> Result<()>;
    async fn exists_by_md5(&self, md5: &str) -> Result<bool>;
    async fn find_cursor_page(
        &self,
        cursor: Option<(chrono::DateTime<chrono::Utc>, PhotoId)>,
        size: u64,
        direction: PageDirection,
    ) -> Result<Vec<Photo>>;
}
```

### 4.3 应用层设计

#### 4.3.1 命令处理器

```rust
// application/handlers/photo_command_handler.rs
use crate::domain::entities::Photo;
use crate::domain::repositories::PhotoRepository;
use crate::domain::events::EventPublisher;
use crate::ports::StoragePort;

pub struct PhotoCommandHandler<R: PhotoRepository, S: StoragePort, E: EventPublisher> {
    repository: R,
    storage: S,
    event_publisher: E,
}

impl<R: PhotoRepository, S: StoragePort, E: EventPublisher> PhotoCommandHandler<R, S, E> {
    pub async fn upload_photo(&self, command: UploadPhotoCommand) -> Result<PhotoId> {
        // 1. 验证文件
        let metadata = self.storage.validate_image(&command.file_data, &command.file_name)?;
        
        // 2. 检查MD5是否存在
        if self.repository.exists_by_md5(&metadata.md5).await? {
            return Err(AppError::bad_request("图片已存在"));
        }
        
        // 3. 上传文件
        let file_id = self.storage.upload(&command.file_data, &metadata).await?;
        
        // 4. 创建领域对象
        let photo = Photo::create(
            command.user_id,
            command.file_name,
            FileInfo::new(file_id, metadata.mime_type, metadata.size),
            ImageDimensions::new(metadata.width, metadata.height),
            metadata.md5,
        );
        
        // 5. 保存到数据库
        self.repository.save(&photo).await?;
        
        // 6. 发布领域事件
        let events = photo.take_events();
        self.event_publisher.publish_all(events).await?;
        
        Ok(photo.id().clone())
    }
}
```

### 4.4 端口设计

#### 4.4.1 存储端口

```rust
// ports/storage_port.rs
use async_trait::async_trait;

#[async_trait]
pub trait StoragePort: Send + Sync {
    async fn upload(&self, file_data: &[u8], metadata: &FileMetadata) -> Result<String>;
    async fn delete(&self, file_id: &str) -> Result<()>;
    async fn delete_batch(&self, file_ids: Vec<String>) -> Result<()>;
    async fn get_download_stream(&self, file_id: &str) -> Result<DownloadStream>;
    async fn download_with_process(&self, file_id: &str, process_param: &str) -> Result<Bytes>;
}
```

### 4.5 基础设施层实现

#### 4.5.1 S3存储适配器

```rust
// infrastructure/storage/s3_storage_adapter.rs
use crate::ports::StoragePort;
use oss::S3Client;

pub struct S3StorageAdapter {
    client: Arc<S3Client>,
}

#[async_trait]
impl StoragePort for S3StorageAdapter {
    async fn upload(&self, file_data: &[u8], metadata: &FileMetadata) -> Result<String> {
        let date_path = chrono::Local::now().format("%Y/%m/%d");
        let uuid = Uuid::new_v4();
        let file_id = format!("photos/{}/{}.{}", date_path, uuid, metadata.format);
        
        self.client.upload(&file_id, file_data, &metadata.mime_type).await?;
        
        Ok(file_id)
    }
    
    // ... 其他方法实现
}
```

## 5. CQRS实现

### 5.1 命令查询分离

- **命令（Commands）**：修改状态的操作，返回成功/失败
- **查询（Queries）**：读取数据的操作，返回数据

### 5.2 读写模型分离

- **写模型**：领域模型，包含业务逻辑
- **读模型**：DTO模型，直接返回给前端

### 5.3 查询优化

- 使用专门的读模型，避免N+1查询
- 可以使用不同的数据库（读写分离）
- 缓存策略更灵活

## 6. 领域事件

### 6.1 事件类型

```rust
// domain/events/photo_events.rs
pub enum PhotoEvent {
    PhotoCreated {
        photo_id: PhotoId,
        user_id: i64,
        created_at: DateTime<Utc>,
    },
    PhotoDeleted {
        photo_id: PhotoId,
        user_id: i64,
        file_id: String,
    },
    PhotoAddedToCollection {
        photo_id: PhotoId,
        collection_id: CollectionId,
    },
    PhotoRemovedFromCollection {
        photo_id: PhotoId,
        collection_id: CollectionId,
    },
}
```

### 6.2 事件处理

- **同步处理**：更新读模型、发送通知
- **异步处理**：更新统计、发送邮件、触发工作流

## 7. 迁移策略

### 7.1 渐进式迁移步骤

1. **阶段1：建立基础结构**
   - 创建新的目录结构
   - 定义端口接口
   - 实现基础设施适配器

2. **阶段2：重构领域层**
   - 提取领域实体和值对象
   - 定义仓储接口
   - 实现领域服务

3. **阶段3：重构应用层**
   - 创建命令和查询
   - 实现命令和查询处理器
   - 处理领域事件

4. **阶段4：重构基础设施层**
   - 实现仓储适配器
   - 重构控制器
   - 集成外部服务

5. **阶段5：测试和验证**
   - 编写单元测试
   - 编写集成测试
   - 性能测试

### 7.2 兼容性策略

- 保持API接口不变
- 新旧代码并存，逐步切换
- 使用特性开关控制新旧逻辑

## 8. 测试策略

### 8.1 单元测试

- **领域层测试**：测试业务逻辑
- **应用层测试**：测试命令和查询处理
- **基础设施层测试**：测试适配器实现

### 8.2 集成测试

- **端到端测试**：测试完整业务流程
- **契约测试**：测试端口和适配器的契约

### 8.3 测试工具

- **Mock框架**：模拟端口实现
- **测试数据库**：内存数据库或测试容器
- **测试数据构建器**：创建测试数据

## 9. 性能考虑

### 9.1 查询优化

- 使用专门的读模型
- 避免N+1查询
- 使用缓存

### 9.2 写入优化

- 批量操作
- 异步处理
- 事件溯源

## 10. 监控和可观测性

### 10.1 指标收集

- 命令执行时间
- 查询响应时间
- 事件处理延迟

### 10.2 日志记录

- 业务操作日志
- 错误日志
- 性能日志

### 10.3 分布式追踪

- 请求链路追踪
- 事件传播追踪

## 11. 风险和缓解措施

### 11.1 技术风险

- **学习曲线**：团队需要学习DDD和六边形架构
- **复杂性增加**：代码结构变得更复杂
- **性能影响**：间接层可能影响性能

### 11.2 缓解措施

- **培训**：提供DDD和六边形架构培训
- **文档**：详细的设计和实现文档
- **代码审查**：严格的代码审查流程
- **性能测试**：定期性能测试和优化

## 12. 成功标准

### 12.1 代码质量

- 代码可读性提高
- 代码重复度降低
- 测试覆盖率提高

### 12.2 开发效率

- 新功能开发时间缩短
- Bug修复时间缩短
- 代码审查时间缩短

### 12.3 系统性能

- 响应时间保持或提高
- 吞吐量保持或提高
- 资源使用保持或降低

## 13. 下一步行动

1. **评审设计文档**：团队评审并确认设计
2. **创建实施计划**：详细的时间线和资源分配
3. **开始试点实施**：选择photo模块开始实施
4. **持续改进**：根据实施反馈调整设计

## 14. 附录

### 14.1 术语表

- **聚合根**：一组相关对象的集合，作为数据修改的单元
- **值对象**：没有唯一标识的对象，通过属性值判断相等性
- **领域事件**：领域中发生的业务事件
- **端口**：定义领域层与外部交互的接口
- **适配器**：实现端口，连接外部技术

### 14.2 参考文献

- 《领域驱动设计》 - Eric Evans
- 《实现领域驱动设计》 - Vaughn Vernon
- 《六边形架构》 - Alistair Cockburn
