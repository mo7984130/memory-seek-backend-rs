//! 运维子命令: 只做一次性操作, 不启动 HTTP 服务。
//!
//! 目前只有「创建默认管理员」: 系统管理员被硬编码为 `auth_user.id = 1`
//! (见 `types_core::AdminId`), 而注册接口要求邮箱验证码 + 邀请码,
//! 因此首次部署后必须用本命令创建这个账号。

use std::io::Read;

use clap::{Args, Subcommand};
use common_core::error::contextual::Result;
use common_core::error::contextual::ext::IntoContextualExt;
use common_core::error::{AppError, ContextualError};
use common_core::time::now;
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, ConnectOptions, Database, DatabaseConnection,
    EntityTrait, PaginatorTrait, QueryFilter,
};
use tracing::info;
use types_identity::auth::user::{ActiveModel, Column, Entity, UserId};
use types_identity::validators::{validate_email, validate_password, validate_username};

use crate::config::AppConfig;

/// 顶层子命令
#[derive(Subcommand)]
pub enum Command {
    /// 管理员账号操作
    Admin {
        #[command(subcommand)]
        action: AdminCommand,
    },
}

/// 管理员子命令
#[derive(Subcommand)]
pub enum AdminCommand {
    /// 创建默认管理员(auth_user.id = 1), 只需在首次部署后执行一次
    Create(AdminCreateArgs),
}

/// `admin create` 参数
#[derive(Args)]
pub struct AdminCreateArgs {
    /// 登录用户名(4-20 位字母、数字、下划线或短横线)
    #[arg(long)]
    pub username: String,

    /// 登录邮箱
    #[arg(long)]
    pub email: String,

    /// 昵称
    #[arg(long, default_value = "管理员")]
    pub nickname: String,

    /// 从标准输入读取密码, 避免密码出现在 shell 历史与进程列表
    #[arg(long)]
    pub password_stdin: bool,
}

/// 执行子命令
pub async fn run(config: &AppConfig, command: Command) -> Result<()> {
    match command {
        Command::Admin { action } => match action {
            AdminCommand::Create(args) => create_admin(config, args).await,
        },
    }
}

/// 创建默认管理员。
///
/// 幂等: 管理员已存在时直接返回成功, 不会覆盖密码。
async fn create_admin(config: &AppConfig, args: AdminCreateArgs) -> Result<()> {
    validate_args(&args)?;
    let password = read_password(args.password_stdin)?;

    let db = connect(config).await?;
    // 表结构同步是幂等的; 保证在从未启动过服务的新库上也能创建
    types_db_init::init_db(&db).await?;

    let total = Entity::find().count(&db).await.map_err(|source| {
        ContextualError::error(
            "admin_count_err",
            "统计用户数量失败",
            source,
            AppError::InternalServerError,
        )
    })?;

    let admin_id = UserId(1);

    if total > 0 {
        let exists = Entity::find()
            .filter(Column::Id.eq(admin_id))
            .one(&db)
            .await
            .map_err(|source| {
                ContextualError::error(
                    "admin_query_err",
                    "查询管理员失败",
                    source,
                    AppError::InternalServerError,
                )
            })?
            .is_some();

        if exists {
            info!("管理员已存在, 跳过创建");
            println!("管理员(id=1)已存在, 无需重复创建");
            return Ok(());
        }

        return Err(ContextualError::error_without_source(
            "admin_missing",
            "数据库已有用户但不存在 id=1 的管理员",
            AppError::bad_request(
                "数据库已有用户但没有 id=1 的管理员, 拒绝自动修复, 请人工确认数据",
            ),
        ));
    }

    let hashed = tokio::task::spawn_blocking(move || constants::PasswordHasher.hash(&password))
        .await
        .into_contextual()??;

    ActiveModel {
        id: Set(admin_id),
        username: Set(args.username.clone()),
        email: Set(args.email.clone()),
        password: Set(hashed),
        nickname: Set(args.nickname.clone()),
        // 管理员的邀请人指向自己: 注册接口里 DRIFTC 邀请码同样映射到 UserId(1)
        inviter: Set(admin_id),
        created_at: Set(now()),
        updated_at: Set(now()),
        ..Default::default()
    }
    .insert(&db)
    .await
    .map_err(|source| {
        ContextualError::error(
            "admin_create_err",
            "创建管理员失败",
            source,
            AppError::InternalServerError,
        )
    })?;

    info!(username = %args.username, email = %args.email, "默认管理员创建成功");
    println!(
        "已创建管理员: id=1 username={} email={}",
        args.username, args.email
    );
    Ok(())
}

/// 建立数据库连接(与服务启动时一致, 但不初始化其它基础设施)
async fn connect(config: &AppConfig) -> Result<DatabaseConnection> {
    let mut options = ConnectOptions::new(&config.database.url);
    options.max_connections(config.database.max_connections);
    Database::connect(options).await.map_err(|source| {
        ContextualError::error(
            "db_connect_err",
            "数据库连接失败",
            source,
            AppError::InternalServerError,
        )
    })
}

/// 校验用户名、邮箱与昵称, 规则与注册接口保持一致
fn validate_args(args: &AdminCreateArgs) -> Result<()> {
    validate_username(&args.username).map_err(|error| invalid_arg("用户名", &error))?;
    validate_email(&args.email).map_err(|error| invalid_arg("邮箱", &error))?;

    let nickname_len = args.nickname.chars().count();
    if !(1..=20).contains(&nickname_len) {
        return Err(ContextualError::warn_without_source(
            "admin_arg_invalid",
            "管理员昵称长度不合法",
            AppError::bad_request("昵称长度需在 1-20 个字符"),
        ));
    }

    Ok(())
}

/// 把校验器的错误转成带字段名的 400
fn invalid_arg(field: &str, error: &validator::ValidationError) -> ContextualError {
    let message = error
        .message
        .as_ref()
        .map(|message| message.to_string())
        .unwrap_or_else(|| error.code.to_string());
    ContextualError::warn_without_source(
        "admin_arg_invalid",
        format!("管理员{field}不合法"),
        AppError::bad_request(format!("{field}不合法: {message}")),
    )
}

/// 从标准输入读取密码(去掉行尾换行), 并按注册规则校验强度
fn read_password(from_stdin: bool) -> Result<String> {
    if !from_stdin {
        return Err(ContextualError::warn_without_source(
            "admin_password_missing",
            "未指定密码来源",
            AppError::bad_request("请通过 --password-stdin 传入密码"),
        ));
    }

    let mut raw = String::new();
    std::io::stdin()
        .read_to_string(&mut raw)
        .map_err(|source| {
            ContextualError::warn(
                "admin_password_read_err",
                "读取标准输入失败",
                source,
                AppError::bad_request("读取密码失败"),
            )
        })?;

    let password = raw.trim_end_matches(['\r', '\n']).to_string();
    validate_password(&password).map_err(|error| invalid_arg("密码", &error))?;
    Ok(password)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(username: &str, email: &str, nickname: &str) -> AdminCreateArgs {
        AdminCreateArgs {
            username: username.to_string(),
            email: email.to_string(),
            nickname: nickname.to_string(),
            password_stdin: true,
        }
    }

    #[test]
    fn accepts_valid_admin_args() {
        assert!(validate_args(&args("admin", "admin@example.com", "管理员")).is_ok());
    }

    #[test]
    fn rejects_invalid_username() {
        assert!(validate_args(&args("ad", "admin@example.com", "管理员")).is_err());
        assert!(validate_args(&args("admin name", "admin@example.com", "管理员")).is_err());
    }

    #[test]
    fn rejects_invalid_email() {
        assert!(validate_args(&args("admin", "not-an-email", "管理员")).is_err());
    }

    #[test]
    fn rejects_invalid_nickname() {
        assert!(validate_args(&args("admin", "admin@example.com", "")).is_err());
        let long = "长".repeat(21);
        assert!(validate_args(&args("admin", "admin@example.com", &long)).is_err());
    }
}
