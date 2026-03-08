use common::error::AppError;
use common::utils::ResultExt;
use aws_config::Region;
use aws_sdk_s3::config::Credentials;
use aws_sdk_s3::Client;
use serde::Deserialize;

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
            .force_path_style(true).build();

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
}