//! HTTP 适配门面:实现已下沉 `common-web`,此处重导出以保持
//! `common::axum::{R, SucR, extractors::ClientIp, ext::ToROkExt}` 等路径不变。

pub use common_web::*;
