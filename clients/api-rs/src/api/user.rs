//! 用户相关 API。
//!
//! 路由前缀:`/user`(需鉴权)。

use crate::client::{Client, Request};
use crate::error::Result;
use types_identity::user::{
    ChangeNicknameParam, ChangePasswordParam, GetUserInfoBatchParam, InviterCodeView,
    UserBriefView, UserInfo,
};
use types_visual_token::VisualTokenStr;

/// 获取当前用户信息。`GET /user/me`
pub async fn get_me(client: &Client) -> Result<UserInfo> {
    client.send_json(Request::get("/user/me")).await
}

/// 退出登录。`POST /user/logout`
pub async fn logout(client: &Client) -> Result<()> {
    client.send_unit(Request::post("/user/logout")).await
}

/// 修改昵称。`PATCH /user/nickname`
pub async fn change_nickname(client: &Client, param: &ChangeNicknameParam) -> Result<String> {
    client
        .send_json(Request::patch("/user/nickname").json(param))
        .await
}

/// 修改密码。`PATCH /user/password`
pub async fn change_password(client: &Client, param: &ChangePasswordParam) -> Result<()> {
    client
        .send_unit(Request::patch("/user/password").json(param))
        .await
}

/// 生成邀请码。`POST /user/inviter-code`
pub async fn generate_inviter_code(client: &Client) -> Result<InviterCodeView> {
    client.send_json(Request::post("/user/inviter-code")).await
}

/// 批量获取用户信息。`POST /user/batch`
pub async fn get_user_info_batch(
    client: &Client,
    param: &GetUserInfoBatchParam,
) -> Result<Vec<Option<UserBriefView>>> {
    client
        .send_json(Request::post("/user/batch").json(param))
        .await
}

/// 上传头像。`PUT /user/avatar`
///
/// 请求体即文件字节(非 multipart);缺省 `Content-Type` 为
/// `application/octet-stream`,后端会按文件头魔数嗅探真实类型。
pub async fn upload_avatar(
    client: &Client,
    bytes: Vec<u8>,
    content_type: Option<String>,
) -> Result<VisualTokenStr> {
    client
        .send_json(Request::put("/user/avatar").bytes(bytes, content_type))
        .await
}
