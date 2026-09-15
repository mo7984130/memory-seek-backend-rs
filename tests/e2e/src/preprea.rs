//! 测试前置准备(prepare): 灌入种子数据。
//!
//! 原 `tests/load/seed/`(seed.sh + seed.sql)的 Rust 化迁移:
//! 幂等灌入账号 / 照片 / 人脸 / 人物 / 时间线统计, 与 server 直连同一数据库。
//! 策略: 每次执行先清空种子数据(保 admin 等非种子数据), 再整批灌入。
//!
//! 前置要求: postgres 服务已启动(`docker compose -f tests/docker-compose.yml up -d --wait`),
//! 且表结构已就绪(server 启动时 `types::db_init::init_db` 自动同步),
//! vector 扩展由 compose 的 postgres-init 服务创建。
//! 数据量等参数见 `e2e.config.yml`(seed 段)。

use sea_orm::{ConnectionTrait, DatabaseConnection, Statement};
use tracing::info;

use crate::config::SeedConfig;
use crate::context::Context;

/// Test123456 的 argon2id 哈希(由 common::utils::HashAlgorithm 生成, 参数 m=16384,t=2,p=1)
const PASS_HASH: &str = "$argon2id$v=19$m=16384,t=2,p=1$T5U+IfQVViaUNr7dhPHmww$CCUS5IsGLNeg0//M+1Iyuwe1izIKPB0oyRud71qofLY";

/// 种子 SQL 语句(占位符 `:NAME` 由 [`SeedConfig`] 替换, 对应原 seed.sh 的 sed 注入)。
/// 前 6 条清理 e2e 自建的相册/评论/点赞与上传照片(保证可重复运行), 其后为清空+灌入。
/// id 规划(避开 admin id=1): auth 用户 id = g+1; photo 用户 id = AUTH_USERS+g+1;
/// user 模块测试池 id = AUTH_USERS + PHOTO_USERS + g(+1)。
const SEED_STATEMENTS: [&str; 22] = [
    // 1. 清理 e2e 自建的评论点赞
    "DELETE FROM photo_comment_like",
    // 2. 清理 e2e 自建的评论
    "DELETE FROM photo_comment",
    // 3. 清理 e2e 自建的照片点赞
    "DELETE FROM photo_photo_like",
    // 4. 清理 e2e 自建的收藏夹照片
    "DELETE FROM photo_collection_photo",
    // 5. 清理 e2e 自建的收藏夹
    "DELETE FROM photo_collection",
    // 6. 清理 e2e 上传的照片(md5 全局唯一, 不清理会阻塞下次同内容上传;
    //    种子照片 file_id 以 seed_file_ 开头, 保留)
    "DELETE FROM photo_photo WHERE file_id NOT LIKE 'seed_file_%'",
    // 7. 清空人脸/人物表(纯 seed 表, 无外键约束, TRUNCATE 重建保证 id 从头开始)
    "TRUNCATE photo_face, photo_person RESTART IDENTITY",
    // 8. 清空种子照片元数据
    "DELETE FROM photo_photo WHERE file_id LIKE 'seed_file_%'",
    // 9. 清空种子账号(保 admin), e2e 自建/测试池账号一并清理保证可重复运行
    "DELETE FROM auth_user WHERE username LIKE 'loadtest_%' OR username LIKE 'e2e_%' OR username LIKE 'uit_%'",
    // 10. 清空当前月时间线统计(由种子数据重建)
    "DELETE FROM photo_timeline_stat WHERE date_str = to_char(now(), 'YYYY-MM')",
    // 11. auth 种子用户
    "
    INSERT INTO auth_user (id, username, email, password, nickname, inviter, created_at, updated_at)
    SELECT g + 1,
           'loadtest_' || g,
           'loadtest_' || g || '@test.com',
           ':PASS_HASH',
           'LoadTest',
           0,
           now(),
           now()
    FROM generate_series(1, :AUTH_USERS) AS g",
    // 12. photo 种子用户
    "
    INSERT INTO auth_user (id, username, email, password, nickname, inviter, created_at, updated_at)
    SELECT (:AUTH_USERS + g + 1),
           'loadtest_photo_' || g,
           'loadtest_photo_' || g || '@test.com',
           ':PASS_HASH',
           'LoadTestPhoto',
           0,
           now(),
           now()
    FROM generate_series(1, :PHOTO_USERS) AS g",
    // 13. 照片元数据(每个 photo 用户预置若干张; file_id 唯一, 无需真实对象存储)
    // created_at 随 (u, p) 递增, 避免所有照片同刻导致分页排序退化
    "
    INSERT INTO photo_photo (user_id, name, size, width, height, mime_type, md5, file_id, created_at, updated_at)
    SELECT (:AUTH_USERS + u + 1),
           'seed_' || u || '_' || p,
           102400,
           400,
           300,
           'image/jpeg',
           lpad((u::bigint * 100000 + p)::text, 32, '0'),
           'seed_file_' || u || '_' || p,
           now() - interval '1 minute' * ((:PHOTO_USERS - u) * :PHOTOS_PER_USER + (:PHOTOS_PER_USER - p)),
           now()
    FROM generate_series(1, :PHOTO_USERS) AS u
    CROSS JOIN generate_series(1, :PHOTOS_PER_USER) AS p",
    // 14. 时间线统计(当前月份, 供 /photo/timeline/stats)
    "
    INSERT INTO photo_timeline_stat (date_str, count, anchor_time, created_at, updated_at)
    VALUES (to_char(now(), 'YYYY-MM'), :PHOTO_COUNT, now(), now(), now())",
    // 15. 人脸(每张 seed 照片 1 张, 初始未分配; embedding 为随机 512 维)
    "
    INSERT INTO photo_face (photo_id, person_id, bbox, landmarks, score, embedding, created_at, updated_at)
    SELECT p.id,
           NULL,
           '[0.1,0.1,0.6,0.9]',
           '[[0.1,0.1],[0.2,0.2],[0.3,0.3],[0.4,0.4],[0.5,0.5]]',
           0.95,
           ('[' || (SELECT string_agg((random() * 2 - 1)::numeric(5,4)::text, ',')
                    FROM generate_series(1, 512)) || ']')::vector,
           now(),
           now()
    FROM photo_photo p
    WHERE p.file_id LIKE 'seed_file_%'",
    // 16. 人物(每 photo 用户 1 个, cover 用其第一张照片; centroid 随机 512 维)
    "
    INSERT INTO photo_person (id, name, name_initials, cover_face_id, cover_photo_id,
                              cover_file_id, cover_face_score, cover_bbox, centroid, face_count, weight,
                              created_at, updated_at)
    SELECT u,
           'Person_' || u,
           'P_' || u,
           (u - 1) * :PHOTOS_PER_USER + 1,
           (u - 1) * :PHOTOS_PER_USER + 1,
           'seed_file_' || u || '_1',
           0.95,
           '[0.1,0.1,0.6,0.9]',
           ('[' || (SELECT string_agg((random() * 2 - 1)::numeric(5,4)::text, ',')
                    FROM generate_series(1, 512)) || ']')::vector,
           :FACES_PER_PERSON,
           :FACES_PER_PERSON * 0.95,
           now(),
           now()
    FROM generate_series(1, :PHOTO_USERS) AS u",
    // 17. 每个用户前 FACES_PER_PERSON 张照片的人脸归属到对应人物
    //     (person u 的照片 id 范围 [(u-1)*PHOTOS_PER_USER+1, u*PHOTOS_PER_USER])
    "
    UPDATE photo_face f
    SET person_id  = ((f.photo_id - 1) / :PHOTOS_PER_USER) + 1,
        updated_at = now()
    WHERE f.photo_id BETWEEN 1 AND :PHOTO_COUNT
      AND ((f.photo_id - 1) % :PHOTOS_PER_USER) + 1 <= :FACES_PER_PERSON",
    // 18. user 模块测试用户池(通用): 供 me/nickname/avatar/logout 及改密负例登录
    "
    INSERT INTO auth_user (id, username, email, password, nickname, inviter, created_at, updated_at)
    SELECT (:AUTH_USERS + :PHOTO_USERS + g + 1),
           'uit_user_' || g,
           'uit_user_' || g || '@test.com',
           ':PASS_HASH',
           'UitUser',
           0,
           now(),
           now()
    FROM generate_series(1, :UIT_USERS) AS g",
    // 19. user 模块测试用户池(改密正例): 改密后旧密码失效, 必须独立于通用池
    "
    INSERT INTO auth_user (id, username, email, password, nickname, inviter, created_at, updated_at)
    SELECT (:AUTH_USERS + :PHOTO_USERS + :UIT_USERS + g + 1),
           'uit_pwd_' || g,
           'uit_pwd_' || g || '@test.com',
           ':PASS_HASH',
           'UitPwd',
           0,
           now(),
           now()
    FROM generate_series(1, :UIT_USERS) AS g",
    // 20. user 模块测试用户池(me 专用): 仅 MeScenario 使用,
    //     与 nickname/avatar/logout 等写场景隔离, 避免并发竞态
    "
    INSERT INTO auth_user (id, username, email, password, nickname, inviter, created_at, updated_at)
    SELECT (:AUTH_USERS + :PHOTO_USERS + :UIT_USERS + :UIT_USERS + g + 1),
           'uit_me_' || g,
           'uit_me_' || g || '@test.com',
           ':PASS_HASH',
           'UitMe',
           0,
           now(),
           now()
    FROM generate_series(1, :UIT_USERS) AS g",
    // 21. user 模块测试用户池(logout 专用): 登出会清除会话, 必须独立于共享池,
    //     避免周期性踢掉同池其它场景的 token
    "
    INSERT INTO auth_user (id, username, email, password, nickname, inviter, created_at, updated_at)
    SELECT (:AUTH_USERS + :PHOTO_USERS + :UIT_USERS + :UIT_USERS + :UIT_USERS + g + 1),
           'uit_logout_' || g,
           'uit_logout_' || g || '@test.com',
           ':PASS_HASH',
           'UitLogout',
           0,
           now(),
           now()
    FROM generate_series(1, :UIT_USERS) AS g",
    // 22. 重置 auth_user 主键序列: 种子显式插入 id 不会推进 BIGSERIAL,
    //     不重置会导致后续业务插入 nextval 撞上种子 id
    "SELECT setval(pg_get_serial_sequence('auth_user', 'id'), (SELECT max(id) FROM auth_user))",
];

/// 灌入种子数据(先清空再灌入, 可重复执行), 返回处理后的 [`Context`] 供后续场景使用。
pub async fn init(ctx: Context, seed: &SeedConfig) -> Result<Context, Box<dyn std::error::Error>> {
    clean_uploaded_objects(&ctx).await?;
    seed_with(&ctx.db, seed).await?;
    Ok(ctx)
}

/// 清理历史 e2e 上传到对象存储的孤儿对象。
///
/// `seed_with` 会删除非种子照片的 DB 记录, 但 S3 对象需另外清理;
/// 清理失败仅告警, 不阻断测试(孤儿对象不影响断言)。
async fn clean_uploaded_objects(ctx: &Context) -> Result<(), Box<dyn std::error::Error>> {
    let stmt = Statement::from_string(
        ctx.db.get_database_backend(),
        "SELECT file_id FROM photo_photo WHERE file_id NOT LIKE 'seed_file_%'".to_owned(),
    );
    for row in ctx.db.query_all_raw(stmt).await? {
        let file_id: String = row.try_get("", "file_id")?;
        if let Err(error) = ctx.s3.delete(&file_id).await {
            tracing::warn!(%file_id, %error, "清理历史上传对象失败(忽略)");
        }
    }
    Ok(())
}

/// 在已建立的连接上灌入种子数据(每次先清空种子数据, 再整批灌入, 可重复执行)。
pub async fn seed_with(
    db: &DatabaseConnection,
    seed: &SeedConfig,
) -> Result<(), Box<dyn std::error::Error>> {
    let stmt = |sql: String| Statement::from_string(db.get_database_backend(), sql);

    info!(
        "seed: auth_users={} photo_users={} photos/user={} faces/person={}",
        seed.auth_users, seed.photo_users, seed.photos_per_user, seed.faces_per_person
    );
    for raw in SEED_STATEMENTS {
        db.execute_raw(stmt(expand(raw, seed))).await?;
    }

    // 汇总校验
    let row = db
        .query_one_raw(stmt(
            "SELECT
                    (SELECT count(*) FROM auth_user  WHERE email LIKE 'loadtest_%') AS loadtest_users,
                    (SELECT count(*) FROM photo_photo)                                AS photos,
                    (SELECT count(*) FROM photo_timeline_stat)                        AS timeline_months,
                    (SELECT count(*) FROM photo_face)                                 AS faces,
                    (SELECT count(*) FROM photo_person)                               AS persons"
                .into(),
        ))
        .await?;
    if let Some(row) = row {
        info!(
            "seed 完成: loadtest_users={} photos={} timeline_months={} faces={} persons={}",
            row.try_get::<i64>("", "loadtest_users")?,
            row.try_get::<i64>("", "photos")?,
            row.try_get::<i64>("", "timeline_months")?,
            row.try_get::<i64>("", "faces")?,
            row.try_get::<i64>("", "persons")?,
        );
    }
    Ok(())
}

/// 替换语句中的 `:NAME` 占位符(对应原 seed.sh 的 sed 注入)。
fn expand(sql: &str, seed: &SeedConfig) -> String {
    sql.replace(":PASS_HASH", PASS_HASH)
        .replace(":AUTH_USERS", &seed.auth_users.to_string())
        .replace(":PHOTO_USERS", &seed.photo_users.to_string())
        .replace(":PHOTOS_PER_USER", &seed.photos_per_user.to_string())
        .replace(":FACES_PER_PERSON", &seed.faces_per_person.to_string())
        .replace(":UIT_USERS", &seed.uit_users.to_string())
        .replace(":PHOTO_COUNT", &seed.photo_count().to_string())
}
