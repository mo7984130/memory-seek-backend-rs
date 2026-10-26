//! API 客户端配置。

/// 客户端配置。
///
/// 只保留与后端交互必需的 `base_url`;超时、重试、TLS 等属于
/// [`Transport`](crate::Transport) 实现的职责,由调用方在注入时自行配置。
#[derive(Debug, Clone)]
pub struct ApiConfig {
    /// API 基础 URL,例如 `http://127.0.0.1:8080/api`。
    ///
    /// 末尾斜杠会被自动裁剪,再与接口路径拼接。
    pub base_url: String,
}

impl ApiConfig {
    /// 以给定基础 URL 构造配置。
    pub fn new(base_url: impl Into<String>) -> Self {
        Self {
            base_url: base_url.into(),
        }
    }
}

impl Default for ApiConfig {
    /// 默认基础 URL 为 `/api`(与 TS 客户端一致,适用于浏览器同源部署)。
    fn default() -> Self {
        Self::new("/api")
    }
}
