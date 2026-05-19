# Service 代码审查进度

## 总览

| Service | 函数数 | 已审查 | 已修复 | 进度 |
|---------|--------|--------|--------|------|
| auth/auth_service.rs | 4+4 | 7 | 3 | ██████████ 100% |
| user/user_service.rs | 7 | 7 | 2 | ██████████ 100% |
| photo/comment_service.rs | 4 | 4 | 1 | ██████████ 100% |
| photo/collection_service.rs | 12 | 0 | 0 | ░░░░░░░░░░ 0% |
| photo/timeline_stat_service.rs | 5 | 0 | 0 | ░░░░░░░░░░ 0% |
| photo/photo_service.rs | 6 | 0 | 0 | ░░░░░░░░░░ 0% |
| photo/feature_service.rs | 5 | 0 | 0 | ░░░░░░░░░░ 0% |
| photo/face_service.rs | 11 | 0 | 0 | ░░░░░░░░░░ 0% |

> auth_service.rs 有 4 个 pub 函数 + 4 个私有辅助函数

## 审查记录

### auth_service.rs

#### ✅ login() — 2026-05-19
- 发现 4 个问题，修复 3 个，跳过 1 个
- 修复: avatar 加密失败加 warn 日志、token 写入改为顺序执行+回滚、注释"效验"→"校验"
- 跳过: spawn_blocking 闭包中的 clone（'static 要求，无法避免）
#### ✅ register() — 2026-05-19
- 无需修复，代码质量良好
- 唯一约束冲突处理、密码哈希、错误传播均正确
#### ✅ send_email_code() — 2026-05-19
- 无需修复，验证码生成、Redis 存储、邮件发送、并发控制均正确
#### ✅ refresh_access_token() — 2026-05-19
- 无需修复

#### ✅ verify_email_verify_code() — 2026-05-19
- 无需修复

#### ✅ verify_inviter_code() — 2026-05-19
- 硬编码 "DriftC" 邀请码已知悉，暂不处理

#### ✅ verify_refresh_token() — 2026-05-19
- 无需修复

### user_service.rs

#### ✅ get_user_info() — 2026-05-19
- 无需修复

#### ✅ generate_inviter_code() — 2026-05-19
- 无需修复（try_seconds unwrap 安全）

#### ✅ change_nickname() — 2026-05-19
- 无需修复

#### ✅ update_avatar() — 2026-05-19
- 修复: 注释"效验"→"校验"

#### ✅ change_password() — 2026-05-19
- 修复: 注释"效验"→"校验"、移除 req.new_password 不必要的 clone

#### ✅ logout() — 2026-05-19
- 无需修复

#### ✅ get_user_info_batch() — 2026-05-19
- 无需修复

### comment_service.rs

#### ✅ get_comment_page() — 2026-05-19
- 修复: 添加 limit 参数校验（正数+上限 100）、提取魔法数字为命名常量、整理函数逻辑注释

#### ✅ publish_comment() — 2026-05-19
- 无需修复

#### ✅ delete_comment() — 2026-05-19
- 无需修复

#### ✅ toggle_like() — 2026-05-19
- 无需修复
