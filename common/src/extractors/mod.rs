/// 请求提取器模块
///
/// 提供自定义的 axum 请求提取器：
/// - `ValidatedJson`: 带 `validator` 校验的 JSON 请求体提取器
/// - `ValidatedQuery`: 带 `validator` 校验的查询参数提取器
/// - `ClientIp`: 客户端 IP 地址提取器，支持 `X-Real-IP` 头和 TCP 连接回退
#[cfg(feature = "validators")]
pub mod validated_json;
#[cfg(feature = "validators")]
pub mod validated_query;
pub mod client_ip;

#[cfg(feature = "validators")]
pub use validated_json::ValidatedJson;
#[cfg(feature = "validators")]
pub use validated_query::ValidatedQuery;
pub use client_ip::ClientIp;
