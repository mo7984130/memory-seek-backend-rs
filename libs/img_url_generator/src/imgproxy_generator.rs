use crate::{ImageUrl, ImageUrlGenerator};
use async_trait::async_trait;
use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
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
    // 使用 HMAC-SHA256 对路径签名，返回 Base64URL 编码的签名字符串
    fn sign(&self, path: &str) -> String {
        let mut mac = Hmac::<Sha256>::new_from_slice(&self.key).expect("Invalid key");
        mac.update(&self.salt);
        mac.update(path.as_bytes());
        URL_SAFE_NO_PAD.encode(mac.finalize().into_bytes())
    }

    // 构建 imgproxy URL：将 S3 源地址编码，拼接处理参数和签名
    fn build_proxy_url(&self, file_id: &str, options: &str, ext: &str) -> ImageUrl {
        let source_url = format!("s3://{}/{}", self.bucket, file_id);
        let encoded = URL_SAFE_NO_PAD.encode(source_url.as_bytes());
        let path = format!("/{}/{}.{}", options, encoded, ext);
        let signature = self.sign(&path);
        let url = format!("{}/{}{}", self.base_url, signature, path);

        ImageUrl {
            url,
            cache_age: CACHE_AGE,
        }
    }
}

#[async_trait]
impl ImageUrlGenerator for ImgProxyGenerator {
    /// 生成缩略图 URL（300x300 填充裁剪，质量 75，WebP 格式）
    ///
    /// # 参数
    /// - `file_id`: S3 中的文件路径/键名
    ///
    /// # 返回
    /// 包含签名 URL 和缓存时长的 `ImageUrl`
    async fn thumbnail(&self, file_id: &str) -> ImageUrl {
        self.build_proxy_url(file_id, "rs:fill:300:300/q:75", "webp")
    }

    /// 生成预览图 URL（宽度 1200px 等比缩放，质量 85，WebP 格式）
    ///
    /// # 参数
    /// - `file_id`: S3 中的文件路径/键名
    ///
    /// # 返回
    /// 包含签名 URL 和缓存时长的 `ImageUrl`
    async fn preview(&self, file_id: &str) -> ImageUrl {
        self.build_proxy_url(file_id, "rs:fit:1200:0/q:85", "webp")
    }

    /// 生成原图 URL（不缩放，保持原始格式）
    ///
    /// # 参数
    /// - `file_id`: S3 中的文件路径/键名
    /// - `extension`: 文件扩展名，决定输出格式
    ///
    /// # 返回
    /// 包含签名 URL 和缓存时长的 `ImageUrl`
    async fn original(&self, file_id: &str, extension: &str) -> ImageUrl {
        self.build_proxy_url(file_id, "rs:fit:0:0", extension)
    }

    /// 生成裁剪图 URL（指定区域裁剪后缩放为正方形，质量 75，WebP 格式）
    ///
    /// # 参数
    /// - `file_id`: S3 中的文件路径/键名
    /// - `x`: 裁剪区域左上角 X 坐标
    /// - `y`: 裁剪区域左上角 Y 坐标
    /// - `w`: 裁剪区域宽度
    /// - `h`: 裁剪区域高度
    /// - `size`: 输出正方形的边长
    ///
    /// # 返回
    /// 包含签名 URL 和缓存时长的 `ImageUrl`
    async fn crop(&self, file_id: &str, x: i32, y: i32, w: i32, h: i32, size: u32) -> ImageUrl {
        let options = format!("rs:fill:{size}:{size}/c:{x}:{y}:{w}:{h}/q:75");
        self.build_proxy_url(file_id, &options, "webp")
    }
}
