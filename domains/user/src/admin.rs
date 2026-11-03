use common_core::{Result, types::CursorPage};
use types_admin::user::AdminUserQuery;
use types_core::UserId;
use types_identity::{auth::user::AdminId, user::UserInfo};

use crate::UserState;

/// 用户域的管理员只读用例。
pub struct UserAdminService;

impl UserAdminService {
    /// 按游标分页查询用户；调用者必须传入已校验的管理员身份。
    pub async fn query_users(
        state: &UserState,
        admin: AdminId,
        query: AdminUserQuery,
    ) -> Result<CursorPage<UserInfo, UserId>> {
        let rows = state
            .repo
            .query_admin_users(admin, query.cursor, query.size + 1)
            .await?;

        let page = CursorPage::from_oversize(rows, query.size).with_next_cursor(|row| row.id);
        let records = page
            .records
            .into_iter()
            .map(|row| row.into_user_info())
            .collect::<common_core::error::contextual::Result<Vec<_>>>()?;

        Ok(CursorPage {
            records,
            next_cursor: page.next_cursor,
            has_more: page.has_more,
        })
    }
}
