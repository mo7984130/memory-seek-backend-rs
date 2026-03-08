use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use hmac::{Hmac, Mac};
use sha2::Sha256;

#[derive(Clone)]
pub struct ImgProxyService {
    base_url: String,
    key: Vec<u8>,
    salt: Vec<u8>,
    bucket: String,
    oss_endpoint: String,
    use_imgproxy: bool,
}

impl ImgProxyService {
    pub fn new(base_url: String, key: &str, salt: &str, bucket: String, oss_endpoint: String, use_imgproxy: bool) -> Self {
        Self {
            base_url,
            key: hex::decode(key).expect("Invalid key hex"),
            salt: hex::decode(salt).expect("Invalid salt hex"),
            bucket,
            oss_endpoint,
            use_imgproxy,
        }
    }

    pub fn generate_url(&self, file_id: &str, options: &str, extension: &str) -> String {
        if self.use_imgproxy {
            self.generate_imgproxy_url(file_id, options, extension)
        } else {
            self.generate_oss_url(file_id, extension)
        }
    }

    fn generate_imgproxy_url(&self, file_id: &str, options: &str, extension: &str) -> String {
        let source_url = format!("s3://{}/{}", self.bucket, file_id);
        let encoded = URL_SAFE_NO_PAD.encode(source_url.as_bytes());
        let path = format!("/{}/{}.{}", options, encoded, extension);
        let signature = self.sign(&path);
        format!("{}/{}{}", self.base_url, signature, path)
    }

    fn generate_oss_url(&self, file_id: &str, extension: &str) -> String {
        format!("{}/{}/{}.{}", self.oss_endpoint, self.bucket, file_id, extension)
    }

    fn sign(&self, path: &str) -> String {
        let mut mac = Hmac::<Sha256>::new_from_slice(&self.key).expect("Invalid key");
        mac.update(&self.salt);
        mac.update(path.as_bytes());
        let result = mac.finalize();
        URL_SAFE_NO_PAD.encode(result.into_bytes())
    }

    pub fn generate_thumbnail_url(&self, file_id: &str) -> String {
        if self.use_imgproxy {
            self.generate_imgproxy_url(file_id, "rs:fill:300:300/q:75", "webp")
        } else {
            self.generate_oss_url(file_id, "webp")
        }
    }

    pub fn generate_preview_url(&self, file_id: &str) -> String {
        if self.use_imgproxy {
            self.generate_imgproxy_url(file_id, "rs:fit:1200:0/q:85", "webp")
        } else {
            self.generate_oss_url(file_id, "webp")
        }
    }

    pub fn generate_original_url(&self, file_id: &str, extension: &str) -> String {
        if self.use_imgproxy {
            self.generate_imgproxy_url(file_id, "rs:fit:0:0", extension)
        } else {
            self.generate_oss_url(file_id, extension)
        }
    }

    pub fn generate_crop_url(&self, file_id: &str, x: i32, y: i32, w: i32, h: i32, size: u32) -> String {
        if self.use_imgproxy {
            let options = format!("crop:{}:{}:nowe:{}:{}/rs:fill:{}:{}", w, h, x, y, size, size);
            self.generate_imgproxy_url(file_id, &options, "webp")
        } else {
            self.generate_oss_url(file_id, "webp")
        }
    }
}
