/// 用户 ID 类型包装
///
/// 对 `i64` 的 newtype 封装，用于在 axum 提取器中明确区分用户 ID 与其他整型参数。
#[derive(Copy, Clone)]
pub struct UserId(pub i64);
