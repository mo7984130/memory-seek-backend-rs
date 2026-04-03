use crate::{ImageUrl, ImageUrlGenerator};
use async_trait::async_trait;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use hmac::{Hmac, Mac};
use sha2::Sha256;

const CACHE_AGE: u32 = 604800;

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
    
    fn build_proxy_url(&self, file_id: &str, options: &str, ext: &str) -> ImageUrl {
        let source_url = format!("s3://{}/{}", self.bucket, file_id);
        let encoded = URL_SAFE_NO_PAD.encode(source_url.as_bytes());
        let path = format!("/{}/{}.{}", options, encoded, ext);
        let signature = self.sign(&path);
        let url = format!("{}/{}{}", self.base_url, signature, path);
        
        ImageUrl { url, cache_age: CACHE_AGE }
    }
}

#[async_trait]
impl ImageUrlGenerator for ImgProxyGenerator {
    async fn thumbnail(&self, file_id: &str) -> ImageUrl {
        self.build_proxy_url(file_id, "rs:fill:300:300/q:75", "webp")
    }

    async fn preview(&self, file_id: &str) -> ImageUrl {
        self.build_proxy_url(file_id, "rs:fit:1200:0/q:85", "webp")
    }

    async fn original(&self, file_id: &str, extension: &str) -> ImageUrl {
        self.build_proxy_url(file_id, "rs:fit:0:0", extension)
    }

    async fn crop(&self, file_id: &str, x: i32, y: i32, w: i32, h: i32, size: u32) -> ImageUrl {
        let options = format!("rs:fill:{size}:{size}/c:{x}:{y}:{w}:{h}/q:75");
        self.build_proxy_url(file_id, &options, "webp")
    }
}
