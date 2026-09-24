//! 领域契约门面:重导出各上下文契约 crate,保持既有 `types::…` 路径不变。
//!
//! - `types-core` 共享内核(强类型 ID、游标、跨上下文枚举)
//! - `types-identity` / `types-visual` / `types-audit` / `types-backup` 各上下文契约
//! - `types-token` 视觉访问令牌协议
//!
//! 数据库 schema 编排在 `types-schema`(实体注册契约在 `types-db-api`)。
//! 新增消费方应直接依赖需要的那一层,而不是本门面 —— 门面只用于迁移期兼容。

/// 视觉上下文契约(`types::visual::*` 路径不变)。
pub use types_visual as visual;

/// 用户身份上下文契约(`types::auth::*` / `types::user::*` / `types::validators::*`
/// 路径不变)。
pub use types_identity::{auth, user, validators};

/// 声明宏(定义在共享内核 `types-core`,此处重导出以保持 `types::in_dto!` /
/// `crate::in_dto!` 等既有路径不变)。
pub use types_core::{in_dto, out_dto, validated_newtype};

/// 审计上下文契约(`types::audit::*` 路径不变)。
pub use types_audit as audit;

/// 备份上下文契约(`types::backup::*` 路径不变)。
pub use types_backup as backup;

/// 键集分页游标(定义在共享内核 `types-core`,此处重导出以保持
/// `types::cursor::*` 路径不变)。
pub use types_core::cursor;

/// ID / 枚举 / 游标的解析错误(定义在共享内核 `types-core`,此处重导出以保持
/// `types::error::*` 路径不变)。
pub use types_core::error;
