use rand::{rng, Rng};

#[inline]
pub fn generate_random_str(len: usize) -> String {
    let mut key = vec![0u8; len / 2];
    rng().fill_bytes(&mut *key);
    hex::encode(key)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 测试生成字符串的长度是否符合预期
    #[test]
    fn test_generate_random_str_length() {
        // 测试生成字符串的长度
        assert_eq!(generate_random_str(8).len(), 8);
        assert_eq!(generate_random_str(16).len(), 16);
        assert_eq!(generate_random_str(32).len(), 32);
        assert_eq!(generate_random_str(64).len(), 64);
    }

    /// 测试生成的字符串是否具有唯一性（不会重复）
    #[test]
    fn test_generate_random_str_uniqueness() {
        // 测试生成的字符串是否唯一
        let str1 = generate_random_str(32);
        let str2 = generate_random_str(32);
        assert_ne!(str1, str2);
    }

    /// 测试生成的字符串是否为有效的十六进制格式
    #[test]
    fn test_generate_random_str_hex_format() {
        // 测试生成的是否为有效的十六进制字符串
        let random_str = generate_random_str(32);
        assert!(random_str.chars().all(|c| c.is_ascii_hexdigit()));
    }

    /// 测试奇数和偶数长度输入的处理
    #[test]
    fn test_generate_random_str_even_length() {
        // 测试奇数长度的情况（实际输出会是偶数，因为 hex 编码）
        let str_odd = generate_random_str(15);
        let str_even = generate_random_str(16);
        // 由于 hex 编码，15/2=7 字节 -> 14 字符，16/2=8 字节 -> 16 字符
        assert_eq!(str_odd.len(), 14);
        assert_eq!(str_even.len(), 16);
    }

    /// 测试输入长度为 0 的边界情况
    #[test]
    fn test_generate_random_str_empty() {
        // 测试长度为 0 的情况
        assert_eq!(generate_random_str(0).len(), 0);
    }

    /// 测试生成字符串的字符分布是否合理
    #[test]
    fn test_generate_random_str_distribution() {
        // 简单测试字符分布（不应该全部相同）
        let random_str = generate_random_str(64);
        let unique_chars: std::collections::HashSet<_> = random_str.chars().collect();
        // 十六进制字符最多 16 个，但至少应该有多个不同的字符
        assert!(unique_chars.len() > 3);
    }
}