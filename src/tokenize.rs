#[derive(Debug, PartialEq)]
pub enum ParseError {
    QuoteMissing,
    UnknownToken,
    FailedToParse,
}

#[derive(Debug, Clone)]
pub enum Quote {
    SingleQuote,
    DoubleQuote,
}

impl Quote {
    pub fn ch (&self) -> char {
        match self {
            Quote::SingleQuote => '\'',
            Quote::DoubleQuote => '"',
        }
    }
}

pub trait Parser<'a, T>: Fn(&'a str) -> Option<(T, &'a str)> {}
impl <'a, T, F> Parser<'a, T> for F where F: Fn(&'a str) -> Option<(T, &'a str)> {}

/**
* word
* accept escape and backslash;
*/
pub fn raw_word(s: &str) -> Option<(&str, &str)> {
    if s.is_empty() {
        return None;
    }
    let mut escape = false;
    for (index, ch) in s.chars().enumerate() {
        if escape {
            escape = false;
            continue;
        }
        if ch == '\\' {
            escape = true;
            continue;
        }
        if ch.is_whitespace() || ch == '\'' || ch == '"' || ch == '>' {
            if index == 0 {
                return None;
            }
            return Some((&s[..index], &s[index..]));
        }
    };
    Some((s, ""))
}


pub fn quoted<'a>(ch: char) -> impl Parser<'a, &'a str> {
    move |s| {
        let mut cursor = s.chars().enumerate();

        cursor.next().filter(|c| c.1 == ch)?;

        let mut escape = false;
        for (index, c) in cursor {
            if escape {
                escape = false;
                continue;
            }
            if c == '\\' {
                escape = true;
                continue;
            }
            if c == ch {
                return Some((&s[..index+1], &s[index+1..]));
            }
        };
        None
    }
}

pub fn many<'a, T>(parser: impl Parser<'a, T>) -> impl Parser<'a, Vec<T>> {
    move |mut s| {
        let mut ret = vec![];
        while let Some((value, rest)) = parser(s) {
            ret.push(value);
            s = rest;
        }
        Some((ret, s))
    }
}

pub fn choice<'a, T>(parser1: impl Parser<'a, T>, parser2: impl Parser<'a, T>) -> impl Parser<'a, T> {
    move |s| parser1(s).or_else(|| parser2(s))
}
#[macro_export]
macro_rules! choice {
    ($parser0:expr, $($parser:expr),* $(,)*) => {{
        let p = $parser0;
        $(
        let p = $crate::tokenize::choice(p, $parser);
        )*
        p
    }};
}
pub fn join<'a, A, B>(parser1: impl Parser<'a, A>, parser2: impl Parser<'a, B>) -> impl Parser<'a, (A, B)> {
    move |s| {
        parser1(s).and_then(|(value1, rest1)| {
            parser2(rest1).map(|(value2, rest2)| ((value1, value2), rest2))
        })
    }
}
#[macro_export]
macro_rules! join {
    ($parser0:expr, $($parser:expr),* $(,)*) => {{
        let p = $parser0;
        $(
        let p = $crate::parser::join(p, $parser);
        )*
        p
    }};
}

pub fn lexeme<'a, T>(parser: impl Parser<'a, T>) -> impl Parser<'a, T> {
    move |s| parser(s.trim_start())
}

/**
* whitespace*
* possibly 
*/
pub fn trim_space(s: &str) -> Option<((), &str)> {
    Some(((), s.trim_start()))
}

/**
* some character
*/
fn word(s: &str) -> Option<(&str, &str)> {
    let elem = choice!(quoted('\''), quoted('"'), raw_word);
    let first = elem(s)?;
    let r = many(elem)(first.1);
    let end = first.0.len() + r.map(|(r, _)| r.iter().fold(0, |sum, x| sum + x.len())).unwrap_or(0);
    Some((&s[..end], &s[end..]))
}

fn redirect(s: &str) -> Option<(&str, &str)> {
    if s.starts_with(">>") {
        Some((&s[..2], &s[2..]))
    } else if s.starts_with(">") {
        Some((&s[..1], &s[1..]))
    } else {
        None
    }
}

pub fn tokenize(src: &str) -> Result<Vec<&str>, ParseError> {
    let r = join(many(choice!(lexeme(word), lexeme(redirect))), trim_space)(src);
    let Some(parsed) = r else {
        return Err(ParseError::FailedToParse);
    };
    if !parsed.1.is_empty() {
        return Err(ParseError::UnknownToken);
    };
    Ok(parsed.0.0)
}

pub fn tokenize_old(src: &str) -> Result<Vec<&str>, ParseError> {
    let mut argv = Vec::<&str>::new();
    let mut start: Option<usize> = None;
    let mut is_in_quote: Option<Quote> = None;
    let mut escape = false;
    for (index, ch) in src.chars().enumerate() {
        // escape
        if escape {
            escape = false;
            continue;
        }
        if ch == '\\' {
            escape = true;
            continue;
        }

        // quotes
        if ch == Quote::SingleQuote.ch() {
            is_in_quote = match is_in_quote {
                None => Some(Quote::SingleQuote),
                Some(Quote::SingleQuote) => None,
                Some(_) => is_in_quote
            };
        } else if ch == Quote::DoubleQuote.ch() {
            is_in_quote = match is_in_quote {
                None => Some(Quote::DoubleQuote),
                Some(Quote::DoubleQuote) => None,
                Some(_) => is_in_quote
            };
        }

        // white spaces
        if ch.is_ascii_whitespace() {
            if is_in_quote.is_some() {
                continue;
            }

            if let Some(start_index) = start {
                let start_i = start_index;
                argv.push(&src[start_i..index]);
                start = None;
                if is_in_quote.is_some() {
                    is_in_quote = None;
                }
                continue;
            }
        } else if start.is_none() {
            start = Some(index);
            continue;
        }
    }

    if let Some(start_index) = start {
        if is_in_quote.is_some() {
            return Err(ParseError::QuoteMissing);
        }
        argv.push(&src[start_index..src.len()]);
    }

    Ok(argv)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_raw_word() {
        let parser = raw_word;
        assert_eq!(parser("abc def"), Some(("abc", " def")));
        assert_eq!(parser("abc'def ghi'"), Some(("abc", "'def ghi'")));
        assert_eq!(parser(r#"abc\ def ghi"#), Some((r#"abc\ def"#, " ghi")));
        assert_eq!(parser(r#"abc\"def ghi"#), Some((r#"abc\"def"#, " ghi")));
    }

    #[test]
    fn test_word() {
        let parser = word;
        assert_eq!(parser("abc def"), Some(("abc", " def")));
        assert_eq!(parser("abc'def ghi' jkl"), Some(("abc'def ghi'", " jkl")));
        assert_eq!(parser(r#""mixed\"quote'shell'\\""#), Some((r#""mixed\"quote'shell'\\""#, "")));
    }
    #[test]
    fn test_word_escape() {
        let parser = word;
        assert_eq!(parser(r#"abc\ def ghi"#), Some((r#"abc\ def"#, " ghi")));
    }

    #[test]
    fn test_lexeme () {
        let parser = lexeme(raw_word);
        assert_eq!(parser("    abc def"), Some(("abc", " def")));
    }

    #[test]
    fn test_many_lexeme_word () {
        let parser = many(lexeme(word));
        assert_eq!(parser("abc def ghi"), Some((vec!["abc", "def", "ghi"], "")));
    }

    #[test]
    fn test_quoted() {
        let parser = quoted('\'');
        assert_eq!(parser(r#"'abc def' ghi"#), Some(("'abc def'", " ghi")));
        assert_eq!(parser(r#"'abc def ghi"#), None);
        assert_eq!(parser(r#"'abc \' def' ghi"#), Some((r#"'abc \' def'"#, " ghi")));
        assert_eq!(parser(r#"'end with escape\\'"#), Some((r#"'end with escape\\'"#, "")));
        assert_eq!(parser(r#"abc def"#), None);
    }

    #[test]
    fn test_redirect() {
        let parser = redirect;
        assert_eq!(parser(">> abc"), Some((">>", " abc")));
        assert_eq!(parser("> abc"), Some((">", " abc")));
        assert_eq!(parser("abc"), None);
    }

    #[test]
    fn test_tokenize() {
        let result = tokenize("a b c").unwrap();
        assert_eq!(result.len(), 3);
        assert_eq!(result[0], "a");
        assert_eq!(result[1], "b");
        assert_eq!(result[2], "c");
    }

    #[test]
    fn test_tokenize_multichar() {
        let result = tokenize("ls -a").unwrap();
        assert_eq!(result.len(), 2);
        assert_eq!(result[0], "ls");
        assert_eq!(result[1], "-a");
    }

    #[test]
    fn test_whitespace() {
        let result = tokenize("    echo    hello world     ").unwrap();
        assert_eq!(result.len(), 3);
        assert_eq!(result[0], "echo");
        assert_eq!(result[1], "hello");
        assert_eq!(result[2], "world");
    }
    #[test]
    fn test_single_quote() {
        let result = tokenize("echo 'abcdef ghijkl'").unwrap();
        assert_eq!(result.len(), 2);
        assert_eq!(result[0], "echo");
        assert_eq!(result[1], "'abcdef ghijkl'");
    }
    #[test]
    fn test_double_quote() {
        let result = tokenize("echo \"abcdef ghijkl\"").unwrap();
        assert_eq!(result.len(), 2);
        assert_eq!(result[0], "echo");
        assert_eq!(result[1], r#""abcdef ghijkl""#);
    }

    #[test]
    #[ignore]
    fn test_missing_quote() {
        let result = tokenize("echo 'a\"b").expect_err("expect missing quote error");
        assert_eq!(result, ParseError::QuoteMissing);
    }

    #[test]
    fn test_tokenize_outside_escape() {
        let result = tokenize("echo a\\ b").unwrap();
        assert_eq!(result.len(), 2);
        assert_eq!(result[0], "echo");
        assert_eq!(result[1], "a\\ b");
    }

    #[test]
    fn test_tokenize_redirect () {
        let result = tokenize("echo a > b").unwrap();
        assert_eq!(result, ["echo", "a", ">", "b"]);
        let result = tokenize("echo a 1> b").unwrap();
        assert_eq!(result, ["echo", "a", "1", ">", "b"]);
        let result = tokenize("echo a 2> b").unwrap();
        assert_eq!(result, ["echo", "a", "2", ">", "b"]);
    }
}
