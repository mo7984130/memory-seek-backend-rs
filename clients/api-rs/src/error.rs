//! 统一错误类型。

use crate::storage::StorageError;
use crate::transport::TransportError;

/// API 客户端错误。
#[derive(Debug, thiserror::Error)]
pub enum ApiError {
    /// 传输层(网络/连接)错误,由调用方的 [`Transport`](crate::Transport) 实现上报。
    #[error("传输错误: {0}")]
    Transport(#[from] TransportError),

    /// 存储层(读写 token 等)错误,由调用方的 [`Storage`](crate::Storage) 实现上报。
    #[error("存储错误: {0}")]
    Storage(#[from] StorageError),

    /// 响应体不是预期的 JSON 结构(或无法解码为期望类型)。
    #[error("响应解析失败: {0}")]
    Decode(#[from] serde_json::Error),

    /// 后端返回的非 2xx 响应:`code` 为 HTTP 响应状态码。
    #[error("接口错误 {code}: {msg}")]
    HttpStatus {
        /// HTTP 响应状态码。
        code: u16,
        /// 后端错误消息(取自响应包壳 `msg`;包壳缺失时为响应体原文)。
        msg: String,
    },

    /// 未登录(存储中没有可用的用户信息)。
    #[error("未登录,请先登录")]
    NotLoggedIn,

    /// 登录已失效:refresh token 缺失或过期,且无法自动刷新。
    #[error("登录已失效,请重新登录")]
    TokenExpired,
}

/// 本 crate 的 `Result` 别名。
pub type Result<T> = std::result::Result<T, ApiError>;
