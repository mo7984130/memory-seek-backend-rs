use pinyin::ToPinyin;

/// 将字符串转换为拼音首字母缩写
///
/// 中文字符取拼音首字母，英文字母保留并转小写，数字被忽略。
///
/// # 参数
/// - `s`: 待转换的字符串
///
/// # 返回
/// 返回拼音首字母组成的字符串
pub fn to_pinyin_initials(s: &str) -> String {
    let mut initials = String::new();

    for c in s.chars() {
        if c.is_ascii_alphabetic() {
            initials.push(c.to_ascii_lowercase());
        } else if c.is_ascii_digit() {
            continue;
        } else if let Some(pinyin) = c.to_pinyin() {
            let pinyin_str = pinyin.plain();
            if let Some(first) = pinyin_str.chars().next() {
                initials.push(first);
            }
        }
    }

    initials
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_to_pinyin_initials_chinese() {
        assert_eq!(to_pinyin_initials("张三"), "zs");
        assert_eq!(to_pinyin_initials("李四一"), "lsy");
    }

    #[test]
    fn test_to_pinyin_initials_mixed() {
        assert_eq!(to_pinyin_initials("Tom张三"), "tomzs");
    }

    #[test]
    fn test_to_pinyin_initials_english() {
        assert_eq!(to_pinyin_initials("Alice"), "alice");
    }
}
