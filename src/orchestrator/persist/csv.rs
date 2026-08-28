pub(crate) fn escape_field(value: &str) -> String {
    if value.contains(['"', ',', '\n', '\r']) {
        format!("\"{}\"", value.replace('"', "\"\""))
    } else {
        value.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clean_field_is_not_quoted() {
        assert_eq!(escape_field("warmup"), "warmup");
        assert_eq!(escape_field("42"), "42");
    }

    #[test]
    fn comma_field_is_quoted() {
        assert_eq!(escape_field("a,b"), "\"a,b\"");
    }

    #[test]
    fn quote_field_is_quoted_with_doubled_quotes() {
        assert_eq!(escape_field("say \"hi\""), "\"say \"\"hi\"\"\"");
    }

    #[test]
    fn newline_field_is_quoted() {
        assert_eq!(escape_field("line1\nline2"), "\"line1\nline2\"");
        assert_eq!(escape_field("line1\r\nline2"), "\"line1\r\nline2\"");
    }

    #[test]
    fn commas_and_quotes_in_one_field_are_all_escaped() {
        assert_eq!(escape_field("a,b,\"c\"\nd"), "\"a,b,\"\"c\"\"\nd\"");
    }
}
