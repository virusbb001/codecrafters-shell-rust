use crate::tokenize::Quote;

struct UnescapeState {
    escape: bool,
    is_in_quote: Option<Quote>
}

fn unescape_inside(ch: char, peek: Option<&char>, mut state: UnescapeState)-> (Option<char>, UnescapeState) {
    if state.escape {
        state.escape = false;
        return (Some(ch), state);
    }
    if ch == '\\' {
        let to_escape = match state.is_in_quote {
            None => true,
            Some(Quote::SingleQuote) => peek.filter(|c| **c == Quote::SingleQuote.ch()).is_some(),
            Some(Quote::DoubleQuote) => peek.filter(|c| **c == Quote::DoubleQuote.ch() || **c == '\\').is_some(),
        };
        if to_escape {
            state.escape = true;
            return (None, state);
        }
    }
    if ch == Quote::SingleQuote.ch() {
        match state.is_in_quote {
            None => {
                state.is_in_quote = Some(Quote::SingleQuote);
                return (None, state);
            },
            Some(Quote::SingleQuote) => {
                state.is_in_quote = None;
                return (None, state);
            },
            Some(_) => {
            }
        }
    }

    if ch == Quote::DoubleQuote.ch() {
        match state.is_in_quote {
            None => {
                state.is_in_quote = Some(Quote::DoubleQuote);
                return (None, state);
            },
            Some(Quote::DoubleQuote) => {
                state.is_in_quote = None;
                return (None, state);
            },
            Some(_) => {
            }
        }
    }
    (Some(ch), state)
}

pub fn unescape(src: &str) -> String {
    let mut result = String::new();
    let mut state = UnescapeState {
        escape: false,
        is_in_quote: None,
    };

    let mut chars = src.chars().peekable();

    while let Some(ch) = chars.next() {
        let unescaped = unescape_inside(ch, chars.peek(), state);
        state = unescaped.1;
        // println!("{} {} {:?} {:?}", ch, state.escape, state.is_in_quote, unescaped.0);
        if let Some(r) = unescaped.0 {
            result.push(r);
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_unescape_none() {
        assert_eq!(unescape("abcdef"), "abcdef");
    }

    #[test]
    fn test_unescape_space() {
        assert_eq!(unescape(r"abc\ def"), r"abc def");
    }

    #[test]
    fn test_unescape_quote() {
        assert_eq!(unescape(r#"abc\"def"#), r#"abc"def"#);
    }

    #[test]
    fn test_double_quote_backslash_space () {
        assert_eq!(unescape(r#""before\   after""#), r#"before\   after"#);
    }

    #[test]
    fn test_double_in_single () {
        assert_eq!(unescape(r#"'exe with "quotes"'"#), r#"exe with "quotes""#);
    }

    #[test]
    fn test_single_in_double () {
        assert_eq!(unescape(r#""exe with 'single quotes'""#), r#"exe with 'single quotes'"#);
    }
    #[test]
    fn test_backslash_within_double_quotes_1 () {
        assert_eq!(
            unescape(r#""hello'script'\\n'world""#),
            r#"hello'script'\n'world"#
        );
    }

    #[test]
    fn test_escape_baclslash_in_double_quote () {
        assert_eq!(unescape(r#""a\\b""#), r#"a\b"#);
    }

    #[test]
    fn test_escape_backslash_in_double_quote_2 () {
        assert_eq!(unescape(r#""hello\"insidequotes"script\""#), r#"hello"insidequotesscript""#);
    }

    #[test]
    fn test_backslash_within_single_quotes () {
        assert_eq!(
            unescape(r#"'shell\\\nscript'"#),
            r#"shell\\\nscript"#
        );
        assert_eq!(
            unescape(r#"'example\"testhello\"shell'"#),
            r#"example\"testhello\"shell"#
        )
    }

}
