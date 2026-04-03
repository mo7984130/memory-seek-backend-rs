use aws_config::Region;
use aws_sdk_s3::config::Credentials;
use aws_sdk_s3::presigning::PresigningConfig;
use aws_sdk_s3::primitives::ByteStream;
use aws_sdk_s3::Client;
use common::error::AppError;
use common::utils::ResultExt;
use serde::Deserialize;
use std::time::Duration;

#[derive(Clone)]
pub struct S3Client {
    client: Client,
    bucket: String,
    public_url: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct S3Config {
    pub endpoint: String,
    pub access_key: String,
    pub secret_key: String,
    pub region: String,
    pub bucket: String,
    pub public_url: String,
    pub force_path_style: bool
}
impl S3Client {
    pub async fn new(
        s3_config: S3Config,
    ) -> Self {
        let credentials = Credentials::new(
            s3_config.access_key,
            s3_config.secret_key,
            None,
            None,
            "static"
        );

        let config = aws_config::defaults(aws_config::BehaviorVersion::latest())
            .region(Region::new(s3_config.region))
            .credentials_provider(credentials)
            .endpoint_url(s3_config.endpoint)
            .load().await;

        let s3_config_builder = aws_sdk_s3::config::Builder::from(&config)
            .force_path_style(s3_config.force_path_style).build();

        let client = Client::from_conf(s3_config_builder);

        Self { client, bucket: s3_config.bucket, public_url: s3_config.public_url.trim_end_matches('/').to_string() }
    }

    pub async fn upload(&self, key: &str, data: Vec<u8>, content_type: &str) -> Result<(), AppError> {
        self.client
            .put_object()
            .bucket(&self.bucket)
            .key(key)
            .body(data.into())
            .set_content_type(Some(content_type.into()))
            .send()
            .await
            .map_internal_err("OSS文件存储失败")?;
        Ok(())
    }

    pub async fn delete(&self, key: &str) -> Result<(), AppError> {
        self.client
            .delete_object()
            .bucket(&self.bucket)
            .key(key)
            .send()
            .await
            .map_internal_err("OSS文件删除失败")?;
        Ok(())
    }

    pub fn get_url(&self, key: &str) -> String {
        format!("{}/{}", self.public_url, key.trim_start_matches('/'))
    }

    pub async fn get_signed_url(&self, key: &str, expires: Duration) -> Result<String, AppError> {
        self.get_signed_url_with_params(key, expires, None).await
    }

    pub async fn get_signed_url_with_params(
        &self,
        key: &str,
        expires: Duration,
        process: Option<String>,
    ) -> Result<String, AppError> {
        let presigning_config = PresigningConfig::expires_in(expires)
            .map_internal_err("签名配置错误")?;

        let builder = self.client.get_object().bucket(&self.bucket).key(key);

        let presigned_res = if let Some(p) = process {
            builder
                .customize()
                // 关键修正：显式标注返回类型 Result<_, std::convert::Infallible>
                .map_request(move |mut req| -> Result<_, std::convert::Infallible> {
                    let uri_str = req.uri().to_string();
                    let connector = if uri_str.contains('?') { "&" } else { "?" };
                    let new_uri_str = format!("{}{}x-oss-process={}", uri_str, connector, p);

                    if let Ok(parsed_uri) = new_uri_str.try_into() {
                        *req.uri_mut() = parsed_uri;
                    }
                    Ok(req)
                })
                .presigned(presigning_config)
                .await
        } else {
            builder.presigned(presigning_config).await
        };

        let output = presigned_res.map_internal_err("OSS 签名失败")?;

        Ok(output.uri().to_string())
    }

    /// 下载文件（流式）
    /// 
    /// # 参数
    /// - `key`: 文件路径
    /// 
    /// # 返回
    /// 返回 ByteStream 流，可直接用于 HTTP 响应
    pub async fn download(&self, key: &str) -> Result<ByteStream, AppError> {
        let output = self.client
            .get_object()
            .bucket(&self.bucket)
            .key(key)
            .send()
            .await
            .map_internal_err("OSS下载失败")?;
        
        Ok(output.body)
    }

    /// 下载文件并应用图片处理参数（流式）
    /// 
    /// # 参数
    /// - `key`: 文件路径
    /// - `process`: OSS图片处理参数，如 "image/resize,w_300"
    /// 
    /// # 返回
    /// 返回 ByteStream 流，可直接用于 HTTP 响应
    pub async fn download_with_process(&self, key: &str, process: &str) -> Result<ByteStream, AppError> {
        let process = process.to_string();
        let output = self.client
            .get_object()
            .bucket(&self.bucket)
            .key(key)
            .customize()
            .map_request(move |mut req| -> Result<_, std::convert::Infallible> {
                let uri_str = req.uri().to_string();
                let connector = if uri_str.contains('?') { "&" } else { "?" };
                let new_uri_str = format!("{}{}x-oss-process={}", uri_str, connector, process);

                if let Ok(parsed_uri) = new_uri_str.try_into() {
                    *req.uri_mut() = parsed_uri;
                }
                Ok(req)
            })
            .send()
            .await
            .map_internal_err("OSS下载失败")?;
        
        Ok(output.body)
    }
}