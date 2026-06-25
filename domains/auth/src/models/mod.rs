use serde::{Deserialize, Serialize};

// 重新导出共享类型
pub use memory_seek_type::auth::*;

// 业务特定类型（如果需要）
#[derive(Debug, Serialize, Deserialize)]
pub struct AuthContext {
    pub user_id: i64,
    pub permissions: Vec<String>,
}

/// 访问令牌信息（业务特定类型）
#[derive(Debug, Serialize, Deserialize)]
pub struct AccessTokenInfo {
    pub token: String,
    pub expire_at: chrono::DateTime<chrono::Utc>,
}
