/// 数据模型模块
///
/// 提供项目公共数据类型：
/// - `ImageToken`: 图片访问 token，支持缩略图、预览图、原图和裁剪图
/// - `UserId`: 用户 ID 的 newtype 封装
pub mod image_token;
mod user_id;

pub use image_token::{FaceBBoxPixels, ImageToken, ImageTokenType};
pub use user_id::UserId;
