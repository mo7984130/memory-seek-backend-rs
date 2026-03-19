use pinyin::ToPinyin;

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
