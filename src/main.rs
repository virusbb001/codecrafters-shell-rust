use std::env;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::{fs, sync::LazyLock};
use std::collections::HashMap;
use std::process::Command;
#[allow(unused_imports)]
use std::io::{self, Write};

type ExitCode = i32;

type BuiltinFunciton = fn(ShellState, &[String])->ShellState;

struct ShellState {
    exit_code: Option<ExitCode>,
    pwd: PathBuf
}
impl ShellState {
    fn default() -> ShellState {
        ShellState {
            exit_code: None,
            pwd: env::current_dir().unwrap()
        }
    }
}

#[derive(Debug, PartialEq)]
enum ParseError {
    QuoteMissing
}

#[derive(Debug, Clone)]
enum Quote {
    SingleQuote,
    DoubleQuote,
}

impl Quote {
    fn ch (&self) -> char {
        match self {
            Quote::SingleQuote => '\'',
            Quote::DoubleQuote => '"',
        }
    }
}

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

fn unescape(src: &str) -> String {
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

fn tokenize(src: &str) -> Result<Vec<&str>, ParseError> {
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

fn parse(src: &str) -> Result<Vec<String>, ParseError> {
    tokenize(src).map(|tokens| tokens.iter().map(|s| unescape(s)).collect())
}

fn echo(state: ShellState, argv: &[String]) -> ShellState {
    let messages = argv.join(" ");
    println!("{}", messages);
    state
}

fn exit(mut state: ShellState, argv: &[String]) -> ShellState {
    let code = argv.first().map(|v| v.parse::<ExitCode>()).unwrap_or(Ok(0));
    if let Err(e) = code {
        println!("{}", e);
    } else if let Ok(code) = code {
        state.exit_code = Some(code);
    }
    state
}

fn type_fn(state: ShellState, argv: &[String]) -> ShellState {
    let Some(cmd) = argv.first() else {
        println!("type [cmd]");
        return state
    };
    if BUILTIN_FUNCITONS.get(cmd.as_str()).is_some() {
        println!("{} is a shell builtin", cmd);
    } else if let Some(cmd_ext) = which_internal(&std::env::var("PATH").unwrap_or("".to_string()), cmd) {
        println!("{} is {}", cmd, cmd_ext.display());
    } else {
        println!("{}: not found", cmd);
    }
    state
}

fn which_internal(path: &str, cmd: &str) -> Option<PathBuf> {
    let path_dirs = path.split(':');
    for dir_name in path_dirs {
        let path = Path::new(dir_name).join(cmd);
        let Ok(metadata) = fs::metadata(path.clone()) else {
            continue;
        };
        if metadata.is_file() && (metadata.permissions().mode() & 0o111 != 0) {
            return Some(path)
        }
    }
    None
}

fn which(state: ShellState, argv: &[String]) -> ShellState {
    let Some(cmd) = argv.first() else {
        println!("which [cmd]");
        return state
    };
    match which_internal(&std::env::var("PATH").unwrap_or("".to_string()), cmd) {
        None => {
            println!("{}: not found", cmd);
        }
        Some(cmd_full) => {
            println!("{} is {}", cmd, cmd_full.as_path().display());
        }
    };
    state
}

fn pwd(state: ShellState, _argv: &[String]) -> ShellState {
    println!("{}", state.pwd.display());
    state
}

fn cd(mut state: ShellState, argv: &[String]) -> ShellState {
    let new_wd = match argv.first() {
        None => {
            env::home_dir()
        }
        Some(dir) => {
            if dir == "~" {
                env::home_dir()
            } else {
                Some(PathBuf::from(dir))
            }
        }
    };
    let Some(new_wd) = new_wd else {
        println!("failed to get new directory");
        return state;
    };

    match fs::canonicalize(state.pwd.join(new_wd.clone())) {
        Ok(path) => {
            state.pwd = path;
        },
        Err(e) => {
            if e.kind() == io::ErrorKind::NotFound {
                println!("cd: {}: No such file or directory", new_wd.display());
            } else {
                println!("Unexpected error: {}, {:?}", e, e.kind());
            }
        }
    }
    state
}

static BUILTIN_FUNCITONS: LazyLock<HashMap<&str, BuiltinFunciton>> = LazyLock::new(|| -> HashMap<&str, BuiltinFunciton> {
    let mut map = HashMap::new();
    map.insert("echo", echo as BuiltinFunciton);
    map.insert("exit", exit as BuiltinFunciton);
    map.insert("type", type_fn as BuiltinFunciton);
    map.insert("which", which as BuiltinFunciton);
    map.insert("pwd", pwd as BuiltinFunciton);
    map.insert("cd", cd as BuiltinFunciton);
    map
});

fn main() {
    let stdin = io::stdin();
    let mut state = ShellState::default();

    // Wait for user input
    while state.exit_code.is_none() {
        print!("$ ");
        io::stdout().flush().unwrap();
        let mut input = String::new();
        stdin.read_line(&mut input).unwrap();
        match parse(&input) {
            Ok(argv) => {
                state = eval(state, &argv);
            },
            Err(e) => {
                println!("{:?}", e);
            }
        }
    }
    std::process::exit(state.exit_code.unwrap());
}

fn eval(state: ShellState, argv: &[String]) -> ShellState{
    let cmd = argv.first();
    match cmd {
        None => state,
        Some(cmd) => {
            if let Some(builtin_fn) = BUILTIN_FUNCITONS.get(cmd.as_str()) {
                builtin_fn(state, &argv[1..])
            } else if let Some(cmd_ext) = which_internal(&std::env::var("PATH").unwrap_or("".to_string()), cmd) {
                let _ = Command::new(cmd_ext).args(&argv[1..])
                    .current_dir(state.pwd.clone())
                    .spawn()
                    .expect("")
                    .wait()
                    ;
                state
            } else {
                println!("{}: command not found", cmd);
                state
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
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
        println!("{:?}", result);
        assert_eq!(result.len(), 2);
        assert_eq!(result[0], "echo");
        assert_eq!(result[1], "'abcdef ghijkl'");
    }
    #[test]
    fn test_double_quote() {
        let result = tokenize("echo \"abcdef ghijkl\"").unwrap();
        println!("{:?}", result);
        assert_eq!(result.len(), 2);
        assert_eq!(result[0], "echo");
        assert_eq!(result[1], r#""abcdef ghijkl""#);
    }

    #[test]
    fn test_missing_quote() {
        let result = tokenize("echo 'a\"b").expect_err("expect missing quote error");
        assert_eq!(result, ParseError::QuoteMissing);
    }

    #[test]
    fn test_tokenize_outside_escape() {
        let result = tokenize("echo a\\ b").unwrap();
        println!("{:?}", result);
        assert_eq!(result.len(), 2);
        assert_eq!(result[0], "echo");
        assert_eq!(result[1], "a\\ b");
    }

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

