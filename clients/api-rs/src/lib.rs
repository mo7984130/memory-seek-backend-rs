//! Memory Seek API 客户端(Rust 实现)。
//!
//! 与 `memory-seek-api-ts` 并列,供 Rust 项目(桌面端等)调用后端。
//!
//! 设计要点:
//! - **运行时无关**:不绑定 tokio 等运行时,异步由调用方提供的执行器驱动。
//! - **HTTP 库无关**:通过 [`Transport`] trait 由调用方注入真实实现;并提供可选
//!   feature `reqwest-transport` 下的开箱即用 `ReqwestTransport`。
//! - **存储可注入**:通过 [`Storage`] trait 由调用方注入 token 存储。
//! - **类型复用**:响应 DTO 直接复用后端 `types-*` 契约 crate,避免定义漂移。
//!
//! # 示例
//!
//! ```ignore
//! use memory_seek_api::{ApiConfig, AuthStorage, Client};
//!
//! // 由调用方提供 Transport / Storage 的具体实现(按值传入,内部自行共享)
//! let client = Client::new(
//!     ApiConfig::new("http://127.0.0.1:8080/api"),
//!     my_transport,
//!     my_storage,
//! );
//!
//! let login = memory_seek_api::api::auth::login(&client, &req).await?;
//! client.auth_storage().set_login_result(&login)?;
//! ```

mod client;
mod config;
mod error;
mod storage;
mod transport;

pub mod api;

pub use client::{Client, RawRequest};
pub use config::ApiConfig;
pub use error::{ApiError, Result};
pub use storage::{AuthStorage, MemoryStorage, Storage, StorageError};

#[cfg(feature = "file-storage")]
pub use storage::FileStorage;
pub use transport::{
    DownloadResponse, HeaderMap, HeaderName, HeaderValue, HttpRequest, HttpResponse, Method,
    StatusCode, Transport, TransportError,
};

#[cfg(feature = "reqwest-transport")]
pub use transport::ReqwestTransport;

// ============================================================
// 复用后端契约类型的再导出
// ============================================================

/// 统一响应壳(后端 `common_core::r::R`)。
///
/// 结构与 [`common_core::r::R`] 一致,直接复用后端契约,避免定义漂移。
pub use common_core::r::R;

/// 分页游标壳(后端 `common_core::types::CursorPage`)。
pub use common_core::types::CursorPage;

/// 跨上下文共享内核:`types-core` 的强类型 ID、枚举与游标。
pub use types_core::cursor::{CountIdCursor, TimeIdCursor};
pub use types_core::{
    AuditId, CollectionId, CollectionVisualId, CommentId, CommentLikeId, FaceId, PageDirection,
    PersonId, TimelineStatId, UserId, VisualId, VisualKind, VisualLikeId,
};

/// 认证 / 用户契约:`types-identity`。
pub use types_identity::auth::{
    LoginRequest, LoginResponse, RefreshAccessTokenResponse, RegisterRequest, SendEmailCodeRequest,
};
pub use types_identity::user::{
    ChangeNicknameParam, ChangePasswordParam, GetUserInfoBatchParam, InviterCodeView,
    UserBriefView, UserIds, UserInfo,
};

/// 视觉契约:`types-visual` + `types-visual-token`。
pub use types_visual::{
    CollectionBriefView, CollectionCreateParam, CollectionUpdateParam, CollectionView,
    CollectionVisualAddBatchParam, CollectionVisualAddBatchResult,
    CollectionVisualRemoveBatchParam, CollectionVisualRemoveBatchResult, CommentContent,
    CommentPublishParam, CommentView, DeleteVisualsParam, ExistsByHashBatchParam, MonthStat,
    UploadVisualParam, VisualIds, VisualTokens, VisualView,
};
pub use types_visual_token::VisualTokenStr;

/// 审计契约:`types-audit`。
pub use types_audit::{AuditGranularity, AuditItem, AuditStatsItem, AuditTopItem};

/// 时间类型(UTC),与后端契约一致。
pub use common_core::time::DateTime;
