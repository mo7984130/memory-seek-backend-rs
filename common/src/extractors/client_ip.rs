use axum::{
    extract::{ConnectInfo, FromRequestParts},
    http::{request::Parts, StatusCode},
};
use axum_client_ip::XRealIp;
use std::net::{IpAddr, SocketAddr};

pub struct ClientIp(pub IpAddr);

impl<S> FromRequestParts<S> for ClientIp
where
    S: Send + Sync,
{
    type Rejection = (StatusCode, &'static str);

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
