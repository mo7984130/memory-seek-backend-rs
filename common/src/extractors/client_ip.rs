use axum::{
    extract::{ConnectInfo, FromRequestParts},
    http::{header, request::Parts},
};
use std::net::{IpAddr, SocketAddr};

use crate::{error::AppError, ext::ResultErrExt};

#[derive(Clone)]
pub struct ClientIp(pub IpAddr);

impl<S> FromRequestParts<S> for ClientIp
where
    S: Send + Sync,
{
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, AppError> {
        // 1. 检查 X-Forwarded-For（代理链，取第一个非代理 IP）
        if let Some(ip) = extract_forwarded_ip(parts) {
            return Ok(ClientIp(ip));
        }

        // 2. 检查 X-Real-IP（单层代理）
        if let Some(ip) = extract_real_ip(parts) {
            return Ok(ClientIp(ip));
        }

        // 3. 回退到直连地址
        extract_connect_info(parts, state).await
    }
}

/// 从 X-Forwarded-For 提取最左侧的客户端 IP
fn extract_forwarded_ip(parts: &Parts) -> Option<IpAddr> {
    parts
        .headers
        .get(header::FORWARDED) // 标准 Forwarded 头
        .or_else(|| parts.headers.get("X-Forwarded-For"))
        .and_then(|v| v.to_str().ok())
        .and_then(|s| {
            s.split(',')
                .next()
                .map(|s| s.trim())
                .and_then(|s| s.parse::<IpAddr>().ok())
                .filter(|ip| !is_private_or_loopback(*ip))
        })
}

/// 从 X-Real-IP 提取 IP
fn extract_real_ip(parts: &Parts) -> Option<IpAddr> {
    parts
        .headers
        .get("X-Real-IP")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.parse::<IpAddr>().ok())
        .filter(|ip| !is_private_or_loopback(*ip))
}

/// 从连接信息提取 IP
async fn extract_connect_info<S>(parts: &mut Parts, state: &S) -> Result<ClientIp, AppError>
where
    S: Send + Sync,
{
    ConnectInfo::<SocketAddr>::from_request_parts(parts, state)
        .await
        .map(|ConnectInfo(addr)| ClientIp(addr.ip()))
        .trace_internal_err(
            "no_client_ip",
            "无法提取客户端 IP，无代理头且未配置 ConnectInfo",
        )
}

/// 判断是否为私有或回环地址
fn is_private_or_loopback(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => v4.is_private() || v4.is_loopback() || v4.is_link_local(),
        IpAddr::V6(v6) => v6.is_loopback() || v6.is_unique_local() || v6.is_multicast(),
    }
}
