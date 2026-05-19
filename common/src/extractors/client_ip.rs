use axum::{
    extract::{ConnectInfo, FromRequestParts},
    http::{request::Parts, StatusCode},
};
use axum_client_ip::XRealIp;
use std::net::{IpAddr, SocketAddr};

/// 客户端 IP 地址提取器
///
/// 优先从 `X-Real-IP` 请求头获取真实客户端 IP，若不存在则回退到 TCP 连接地址。
/// 可直接作为 axum handler 参数提取客户端 IP。
pub struct ClientIp(pub IpAddr);

impl<S> FromRequestParts<S> for ClientIp
where
    S: Send + Sync,
{
    type Rejection = (StatusCode, &'static str);

    /// 从请求中提取客户端 IP 地址
    ///
    /// # 参数
    /// - `parts`: 请求头部分，用于读取 `X-Real-IP` 头或连接信息
    /// - `state`: axum 应用状态
    ///
    /// # 返回
    /// 返回包含客户端 IP 的 `ClientIp`
    ///
    /// # 错误
    /// - `(StatusCode::INTERNAL_SERVER_ERROR, _)`: 路由未配置 `ConnectInfo` 且无 `X-Real-IP` 头
    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let x_real_ip = XRealIp::from_request_parts(parts, state)
            .await
            .ok();

        if let Some(XRealIp(ip)) = x_real_ip {
            return Ok(ClientIp(ip));
        }

        let ConnectInfo(addr) = ConnectInfo::<SocketAddr>::from_request_parts(parts, state)
            .await
            .map_err(|_| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Missing ConnectInfo in router setup",
                )
            })?;

        Ok(ClientIp(addr.ip()))
    }
}
