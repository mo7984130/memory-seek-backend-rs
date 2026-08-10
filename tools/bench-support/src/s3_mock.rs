//! 嵌入式内存 S3 mock
//!
//! 基于 axum 的进程内 S3 兼容服务,实现 `PUT/GET/DELETE /{bucket}/{*key}` 与
//! `PUT /{bucket}`(自动建桶),数据保存在内存 `HashMap` 中。忽略 AWS SigV4 验签,
//! 客户端侧签名头原样忽略,按 HTTP 状态码返回(满足 rust-s3 的 2xx 判定)。
//!
//! 用法:
//! ```ignore
//! let mock = S3Mock::start("bench-bucket").await;
//! let client = oss::S3Client::new(&mock.s3_config());
//! ```

use axum::body::Bytes;
use axum::extract::{DefaultBodyLimit, Path, State};
use axum::http::StatusCode;
use axum::routing::{delete, get, put};
use axum::Router;
use oss::S3Config;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use tokio::net::TcpListener;

#[derive(Clone, Default)]
struct Store(Arc<Mutex<HashMap<String, Vec<u8>>>>);

/// 内存 S3 mock 服务器
pub struct S3Mock {
    addr: SocketAddr,
    store: Store,
    bucket: String,
    _server: tokio::task::JoinHandle<()>,
}

impl S3Mock {
    /// 在随机端口启动 S3 mock,绑定 bucket
    pub async fn start(bucket: impl Into<String>) -> Self {
        let store = Store::default();
        let router = Router::new()
            .route("/{bucket}", put(create_bucket))
            .route("/{bucket}/{*key}", put(put_object))
            .route("/{bucket}/{*key}", get(get_object))
            .route("/{bucket}/{*key}", delete(delete_object))
            .layer(DefaultBodyLimit::max(512 * 1024 * 1024))
            .with_state(store.clone());

        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            axum::serve(listener, router).await.unwrap();
        });

        Self {
            addr,
            store,
            bucket: bucket.into(),
            _server: server,
        }
    }

    /// 访问端点(`http://127.0.0.1:<port>`)
    pub fn endpoint(&self) -> String {
        format!("http://{}", self.addr)
    }

    /// 绑定的 bucket 名
    pub fn bucket(&self) -> &str {
        &self.bucket
    }

    /// 生成指向该 mock 的 `oss::S3Config`
    pub fn s3_config(&self) -> S3Config {
        S3Config {
            endpoint: self.endpoint(),
            access_key: "bench".into(),
            secret_key: "bench".into(),
            region: "us-east-1".into(),
            bucket: self.bucket.clone(),
            public_url: Some(self.endpoint()),
            force_path_style: true,
        }
    }

    /// 清空当前 bucket 下所有对象
    pub fn clear(&self) {
        let prefix = format!("{}/", self.bucket);
        self.store
            .0
            .lock()
            .unwrap()
            .retain(|k, _| !k.starts_with(&prefix));
    }
}

impl Drop for S3Mock {
    fn drop(&mut self) {
        self._server.abort();
    }
}

fn storage_key(bucket: &str, key: &str) -> String {
    format!("{}/{}", bucket, key)
}

async fn create_bucket(State(_store): State<Store>, Path(_bucket): Path<String>) -> StatusCode {
    StatusCode::OK
}

async fn put_object(
    State(store): State<Store>,
    Path((bucket, key)): Path<(String, String)>,
    body: Bytes,
) -> StatusCode {
    store
        .0
        .lock()
        .unwrap()
        .insert(storage_key(&bucket, &key), body.to_vec());
    StatusCode::OK
}

async fn get_object(
    State(store): State<Store>,
    Path((bucket, key)): Path<(String, String)>,
) -> Result<Bytes, StatusCode> {
    store
        .0
        .lock()
        .unwrap()
        .get(&storage_key(&bucket, &key))
        .cloned()
        .map(Bytes::from)
        .ok_or(StatusCode::NOT_FOUND)
}

async fn delete_object(
    State(store): State<Store>,
    Path((bucket, key)): Path<(String, String)>,
) -> StatusCode {
    store
        .0
        .lock()
        .unwrap()
        .remove(&storage_key(&bucket, &key));
    StatusCode::NO_CONTENT
}
