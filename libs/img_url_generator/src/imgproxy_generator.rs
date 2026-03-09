use async_trait::async_trait;
use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use hmac::{Hmac, Mac};
use sha2::Sha256;
use crate::ImageUrlGenerator;

pub struct ImgProxyGenerator {
    pub base_url: String,
    pub key: Vec<u8>,
    pub salt: Vec<u8>,
    pub bucket: String,
}

impl ImgProxyGenerator {
    fn sign(&self, path: &str) -> String {
        let mut mac = Hmac::<Sha256>::new_from_slice(&self.key).expect("Invalid key");
        mac.update(&self.salt);
        mac.update(path.as_bytes());
        URL_SAFE_NO_PAD.encode(mac.finalize().into_bytes())
    }
    
    async fn build_proxy_url(&self, file_id: &str, options: &str, ext: &str) -> String {
        let source_url = format!("s3://{}/{}", self.bucket, file_id);
        let encoded = URL_SAFE_NO_PAD.encode(source_url.as_bytes());
        let path = format!("/{}/{}.{}", options, encoded, ext);
        let signature = self.sign(&path);
        format!("{}/{}{}", self.base_url, signature, path)
    }
}

#[async_trait]
impl ImageUrlGenerator for ImgProxyGenerator {
    async fn thumbnail(&self, file_id: String) -> String {
        self.build_proxy_url(&file_id, "rs:fill:300:300/q:75", "webp").await
    }

    async fn preview(&self, file_id: String) -> String {
        self.build_proxy_url(&file_id, "rs:fit:1200:0/q:85", "webp").await
    }

    async fn original(&self, file_id: String, extension: String) -> String {
        self.build_proxy_url(&file_id, "rs:fit:0:0", &extension).await
    }

    async fn crop(&self, file_id: String, x: i32, y: i32, w: i32, h: i32, size: u32) -> String {
        let options = format!("rs:fill:{size}:{size}/c:{x}:{y}:{w}:{h}/q:75");
        self.build_proxy_url(&file_id, &options, "webp").await
    }
}