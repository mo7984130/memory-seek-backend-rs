/// 密码验证最大并发数，`0` 表示由运行时根据 CPU 核心数自动计算。
const PASSWORD_VERIFY_MAX_CONCURRENCY: usize = 0;

/// 获取密码验证的最大并发度
///
/// 当 [`PASSWORD_VERIFY_MAX_CONCURRENCY`] 为 `0` 时，自动取 CPU 核心数的一半（最少为 1）；
/// 否则返回配置的固定值。
///
/// # 返回
/// 密码验证允许的最大并发任务数
pub fn get_password_verify_max_concurrency() -> usize {
    if PASSWORD_VERIFY_MAX_CONCURRENCY == 0 {
        (num_cpus::get() / 2).max(1)
    } else {
        PASSWORD_VERIFY_MAX_CONCURRENCY
    }
}
