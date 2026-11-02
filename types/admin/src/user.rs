use serde::{Deserialize, Serialize};
use types_core::UserId;
use validator::Validate;

fn default_page_size() -> u64 {
    32
}

/// 管理员只读查询用户的分页参数。
#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
#[serde(rename_all = "camelCase")]
pub struct AdminUserQuery {
    /// 上一页最后一条记录的用户 ID。
    pub cursor: Option<UserId>,
    /// 每页数量，默认 32，范围 1 到 100。
    #[serde(default = "default_page_size")]
    #[validate(range(min = 1, max = 100, message = "size 在 1 到 100 之间"))]
    pub size: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_page_size_is_valid() {
        let query = AdminUserQuery {
            cursor: None,
            size: default_page_size(),
        };
        assert!(query.validate().is_ok());
    }

    #[test]
    fn page_size_is_bounded() {
        let query = AdminUserQuery {
            cursor: None,
            size: 101,
        };
        assert!(query.validate().is_err());
    }
}
