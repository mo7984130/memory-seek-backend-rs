use bench_support::s3_mock::S3Mock;

/// 验证嵌入式 S3 mock 能被 `oss::S3Client` 真实驱动(upload/download/delete)
#[tokio::test]
async fn s3_mock_put_get_delete() {
    let mock = S3Mock::start("bench-bucket").await;
    let client = oss::S3Client::new(&mock.s3_config());

    client
        .upload("dir/obj.txt", &b"hello bench"[..], "text/plain")
        .await
        .unwrap();
    let bytes = client.download("dir/obj.txt").await.unwrap();
    assert_eq!(bytes.as_ref(), b"hello bench");

    client.delete("dir/obj.txt").await.unwrap();
    assert!(client.download("dir/obj.txt").await.is_err());
}

/// 验证 mock.clear() 清空 bucket 对象
#[tokio::test]
async fn s3_mock_clear() {
    let mock = S3Mock::start("bench-bucket").await;
    let client = oss::S3Client::new(&mock.s3_config());

    client
        .upload("a", &b"x"[..], "text/plain")
        .await
        .unwrap();
    mock.clear();
    assert!(client.download("a").await.is_err());
}
