//! 系统级端点(健康检查等)的测试。

mod common;

use futures::executor::block_on;
use memory_seek_api::Method;

use common::{client, header, ok_text};

#[test]
fn health_returns_plain_text_without_auth() {
    let (client, transport) = client(vec![ok_text("ok")]);
    let health = block_on(memory_seek_api::api::system::health(&client)).unwrap();
    assert_eq!(health, "ok");

    let requests = transport.requests();
    assert_eq!(requests[0].method, Method::GET);
    assert_eq!(requests[0].url, "/api/health");
    assert!(header(&requests[0], "authorization").is_none());
}
