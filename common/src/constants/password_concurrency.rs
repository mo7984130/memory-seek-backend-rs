const PASSWORD_VERIFY_MAX_CONCURRENCY: usize = 0;

pub fn get_password_verify_max_concurrency() -> usize {
    if PASSWORD_VERIFY_MAX_CONCURRENCY == 0 {
        (num_cpus::get() / 2).max(1)
    } else {
        PASSWORD_VERIFY_MAX_CONCURRENCY
    }
}
