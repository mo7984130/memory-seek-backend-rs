pub mod common {
    use axum::body::Body;
    use axum::http::{Request, StatusCode, header};
    use serde_json::Value;
    use tower::ServiceExt;

    use super::super::auth;

    /// Minimal valid 1x1 PNG bytes for testing
    pub const MINIMAL_JPEG: &[u8] = &[
        0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x48, 0x44,
        0x52, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x02, 0x00, 0x00, 0x00, 0x90,
        0x77, 0x53, 0xDE, 0x00, 0x00, 0x00, 0x0C, 0x49, 0x44, 0x41, 0x54, 0x08, 0xD7, 0x63, 0xF8,
        0xFF, 0x7F, 0x00, 0x05, 0xFE, 0x02, 0xFE, 0xDC, 0x44, 0x48, 0x30, 0x00, 0x00, 0x00, 0x00,
        0x49, 0x45, 0x4E, 0x44, 0xAE, 0x42, 0x60, 0x82,
    ];

    /// Build a multipart upload request with authentication
    fn multipart_upload_request(
        uri: &str,
        user: &auth::TestUser,
        file_data: &[u8],
        filename: &str,
    ) -> Request<Body> {
        let boundary = "----testboundary";
        let body = format!(
            "--{boundary}\r\n\
             Content-Disposition: form-data; name=\"file\"; filename=\"{filename}\"\r\n\
             Content-Type: image/png\r\n\r\n"
        );
        let mut body_bytes = body.into_bytes();
        body_bytes.extend_from_slice(file_data);
        body_bytes.extend_from_slice(format!("\r\n--{boundary}--\r\n").as_bytes());

        Request::builder()
            .method("POST")
            .uri(uri)
            .header(
                header::CONTENT_TYPE,
                format!("multipart/form-data; boundary={boundary}"),
            )
            .header(
                "Authorization",
                format!("Bearer {} {}", user.id, user.access_token),
            )
            .body(Body::from(body_bytes))
            .unwrap()
    }

    /// Upload a photo and return the photo_id string.
    ///
    /// Returns `None` if S3/MinIO is not available (500).
    pub async fn upload_photo(app: &axum::Router, user: &auth::TestUser) -> Option<String> {
        let req = multipart_upload_request("/photo", user, MINIMAL_JPEG, "test.png");
        let res = app.clone().oneshot(req).await.unwrap();

        if res.status() == StatusCode::INTERNAL_SERVER_ERROR {
            return None;
        }

        assert_eq!(res.status(), StatusCode::OK, "上传照片失败");

        let body_bytes = axum::body::to_bytes(res.into_body(), 1024 * 1024)
            .await
            .unwrap();
        let json: Value = serde_json::from_slice(&body_bytes).unwrap();
        assert_eq!(json["code"], 200);

        Some(json["data"]["id"].as_str().unwrap().to_string())
    }
}
