use chrono::{DateTime, Utc};
use common::error::{AppError, DeferredError, DeferredResult};
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, Condition, ConnectionTrait, DbErr,
    EntityTrait, FromQueryResult, QueryFilter, QuerySelect, RuntimeErr, sea_query::Expr,
};
use types::auth::user::{ActiveModel, Column, Entity, UserId, UserRecord};

pub struct AuthMapper;

pub struct AuthInsertParam {
    pub username: String,
    pub email: String,
    pub password: String,
    pub nickname: String,
    pub inviter: UserId,
}
// 校验 refresh_token：从数据库查询用户的 refresh_token 并验证匹配性和有效期
#[derive(FromQueryResult)]
pub struct RefreshTokenValidation {
    pub refresh_token: Option<String>,
    pub refresh_token_expire_at: Option<DateTime<Utc>>,
}
// 创建
impl AuthMapper {
    pub async fn insert(
        db: &impl ConnectionTrait,
        param: AuthInsertParam,
    ) -> DeferredResult<UserRecord> {
        let model = ActiveModel {
            username: Set(param.username),
            email: Set(param.email),
            password: Set(param.password),
            nickname: Set(param.nickname),
            inviter: Set(param.inviter),
            ..Default::default()
        }
        .insert(db)
        .await
        .map_err(Self::handle_user_insert_err)?;
        Ok(UserRecord::from(model))
    }

    /// 将 SeaORM 插入用户时的 DbErr 转换为 AppError
    fn handle_user_insert_err(e: DbErr) -> DeferredError {
        // 解析 PostgreSQL 唯一约束冲突 (23505)
        if let DbErr::Query(RuntimeErr::SqlxError(ref sqlx_err)) = e
            && let Some(pg_err) = sqlx_err.as_database_error()
            && pg_err.code() == Some("23505".into())
        {
            let detail = pg_err.to_string().to_lowercase();
            let (reason, msg) = if detail.contains("username") {
                ("username_existed", "该用户名已被占用")
            } else if detail.contains("email") {
                ("email_existed", "该邮箱已被注册")
            } else {
                ("row_existed", "记录已存在")
            };

            return DeferredError::warn(reason, "注册失败", e, AppError::bad_request(msg));
        }

        DeferredError::error(
            "register_err",
            "用户注册时发生数据库异常",
            e,
            AppError::InternalServerError,
        )
    }

    pub async fn query_refresh_token(
        db: &impl ConnectionTrait,
        user_id: UserId,
    ) -> DeferredResult<Option<RefreshTokenValidation>> {
        Ok(Entity::find()
            .select_only()
            .column(Column::RefreshToken)
            .column(Column::RefreshTokenExpireAt)
            .filter(Column::Id.eq(user_id))
            .into_model::<RefreshTokenValidation>()
            .one(db)
            .await?)
    }
}

// 修改
impl AuthMapper {
    pub async fn update_password(
        db: &impl ConnectionTrait,
        user_id: UserId,
        password: &str,
    ) -> DeferredResult<u64> {
        Ok(Entity::update_many()
            .filter(Column::Id.eq(user_id))
            .col_expr(Column::Password, Expr::value(password))
            .exec(db)
            .await?
            .rows_affected)
    }

    pub async fn update_refresh_token(
        db: &impl ConnectionTrait,
        user_id: UserId,
        refresh_token: String,
        refresh_token_expires_at: DateTime<Utc>,
    ) -> DeferredResult<UserRecord> {
        let model = ActiveModel {
            id: Set(user_id),
            refresh_token: Set(Some(refresh_token)),
            refresh_token_expire_at: Set(Some(refresh_token_expires_at)),
            ..Default::default()
        }
        .update(db)
        .await?;
        Ok(UserRecord::from(model))
    }
}

#[derive(Debug, FromQueryResult)]
pub struct UserPasswordId {
    pub id: UserId,
    pub password: String,
}
// 查询
impl AuthMapper {
    pub async fn query_by_account(
        db: &impl ConnectionTrait,
        account: &str,
    ) -> DeferredResult<Option<UserPasswordId>> {
        Ok(Entity::find()
            .select_only()
            .column(Column::Id)
            .column(Column::Password)
            .filter(
                Condition::any()
                    .add(Column::Username.eq(account))
                    .add(Column::Email.eq(account)),
            )
            .into_model::<UserPasswordId>()
            .one(db)
            .await?)
    }
}

// 删除
impl AuthMapper {}
