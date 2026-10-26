pub fn truncate_utf8(input: &str, max_bytes: usize) -> String {
    if input.len() <= max_bytes {
        return input.to_owned();
    }
    if max_bytes == 0 {
        return String::new();
    }

    let mut end = max_bytes;
    while end > 0 && !input.is_char_boundary(end) {
        end -= 1;
    }
    input[..end].to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn short_text_is_unchanged() {
        assert_eq!(truncate_utf8("hello", 16), "hello");
    }

    #[test]
    fn ascii_is_cut_at_the_limit() {
        let out = truncate_utf8("abcdefghij", 4);
        assert_eq!(out, "abcd");
        assert!(out.len() <= 4);
    }

    #[test]
    fn multibyte_is_never_split() {
        let input = "错误信息：无法启动";
        let out = truncate_utf8(input, 10);
        assert!(out.len() <= 10);
        assert!(input.is_char_boundary(out.len()));
    }

    #[test]
    fn zero_limit_is_empty() {
        assert_eq!(truncate_utf8("abc", 0), "");
    }
}
