use crate::tokenize::ParseError;
use crate::tokenize::tokenize;
use crate::unescape::unescape;
use std::env;
use std::fs::File;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::{fs, sync::LazyLock};
use std::collections::HashMap;
use std::process::Command;
#[allow(unused_imports)]
use std::io::{self, Write};

mod tokenize;
mod unescape;

type ExitCode = i32;

type BuiltinFunction = fn(ShellState, &[String], Box<dyn Write>)->ShellState;

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

#[derive(PartialEq, Debug)]
enum RedirMode {
    Write,
    Append
}

struct Proc<'a> {
    exec: &'a str,
    argv: Vec<String>,
    stdout: Option<&'a str>,
    stdout_mode: RedirMode,
    stderr: Option<&'a str>,
    stderr_mode: RedirMode,
}

fn parse(src: &str) -> Result<Vec<String>, ParseError> {
    tokenize(src).map(|tokens| tokens.iter().map(|s| unescape(s)).collect())
}

enum ToRedirect {
    Stdout,
    Stderr,
}

fn words2proc(argv: &[String]) -> Option<Proc<'_>> {
    let exec = argv.first()?;
    let mut cursor = argv[1..].iter().enumerate().peekable();
    let mut to_redirect: Option<ToRedirect> = None;
    
    let mut proc = Proc {
        exec,
        argv: Vec::<String>::new(),
        stdout: None,
        stdout_mode: RedirMode::Write,
        stderr: None,
        stderr_mode: RedirMode::Write,
    };

    while let Some((_index, word)) = cursor.next() {
        if word == "1" || word == "2" {
            let next = cursor.peek();
            let next_is_redirect = next.filter(|(_, w)| *w == ">" || *w == ">>").is_some();
            if next_is_redirect {
                to_redirect = match word.as_str() {
                    "1" => Some(ToRedirect::Stdout),
                    "2" => Some(ToRedirect::Stderr),
                    _ => panic!()
                };
                continue;
            }
        } else if word == ">" {
            let target = cursor.next().unwrap().1;
            match to_redirect.as_ref().unwrap_or(&ToRedirect::Stdout) {
                ToRedirect::Stdout => {
                    proc.stdout = Some(target);
                    proc.stdout_mode = RedirMode::Write;
                },
                ToRedirect::Stderr => {
                    proc.stderr = Some(target);
                    proc.stderr_mode = RedirMode::Write;
                },
            }
            continue;
        }

        if word == ">>" {
            let target = cursor.next().unwrap().1;
            match to_redirect.as_ref().unwrap_or(&ToRedirect::Stdout) {
                ToRedirect::Stdout => {
                    proc.stdout = Some(target);
                    proc.stdout_mode = RedirMode::Append;
                },
                ToRedirect::Stderr => {
                    proc.stderr = Some(target);
                    proc.stderr_mode = RedirMode::Append;
                },
            }
            continue;
        }

        proc.argv.push(word.to_string());
    }

    Some(proc)
}

fn echo(state: ShellState, argv: &[String], mut stdout: Box<dyn Write>) -> ShellState {
    let messages = argv.join(" ");
    stdout.write_all(format!("{}\n", messages).as_bytes()).expect("should success to write");
    state
}

fn exit(mut state: ShellState, argv: &[String], _: Box<dyn Write>) -> ShellState {
    let code = argv.first().map(|v| v.parse::<ExitCode>()).unwrap_or(Ok(0));
    if let Err(e) = code {
        println!("{}", e);
    } else if let Ok(code) = code {
        state.exit_code = Some(code);
    }
    state
}

fn type_fn(state: ShellState, argv: &[String], _: Box<dyn Write>) -> ShellState {
    let Some(cmd) = argv.first() else {
        println!("type [cmd]");
        return state
    };
    if BUILTIN_FUNCITONS.get((*cmd).as_str()).is_some() {
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

fn which(state: ShellState, argv: &[String], _: Box<dyn Write>) -> ShellState {
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

fn pwd(state: ShellState, _argv: &[String], _: Box<dyn Write>) -> ShellState {
    println!("{}", state.pwd.display());
    state
}

fn cd(mut state: ShellState, argv: &[String], _: Box<dyn Write>) -> ShellState {
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

static BUILTIN_FUNCITONS: LazyLock<HashMap<&str, BuiltinFunction>> = LazyLock::new(|| -> HashMap<&str, BuiltinFunction> {
    let mut map = HashMap::new();
    map.insert("echo", echo as BuiltinFunction);
    map.insert("exit", exit as BuiltinFunction);
    map.insert("type", type_fn as BuiltinFunction);
    map.insert("which", which as BuiltinFunction);
    map.insert("pwd", pwd as BuiltinFunction);
    map.insert("cd", cd as BuiltinFunction);
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
    let proc = words2proc(argv);
    match proc {
        None => state,
        Some(proc) => {
            if let Some(builtin_fn) = BUILTIN_FUNCITONS.get(proc.exec) {
                let stdout: Box<dyn Write> = match proc.stdout {
                    None => Box::new(std::io::stdout()),
                    Some(filename) => {
                        let filename = state.pwd.join(filename);
                        match proc.stdout_mode {
                            RedirMode::Write => Box::new(File::create(filename).unwrap()),
                            RedirMode::Append => Box::new(File::options()
                                .append(true)
                                .open(filename)
                                .unwrap())
                        }

                    }
                };
                builtin_fn(state, &proc.argv, stdout)
            } else if let Some(cmd_ext) = which_internal(&std::env::var("PATH").unwrap_or("".to_string()), proc.exec) {
                let mut cmd = Command::new(cmd_ext);
                cmd.args(proc.argv)
                    .current_dir(state.pwd.clone());
                if let Some(stdout) = proc.stdout {
                    let filename = state.pwd.join(stdout);
                    let f = match proc.stdout_mode {
                        RedirMode::Write => File::create(filename).unwrap(),
                        RedirMode::Append => File::options()
                            .append(true)
                            .open(filename)
                            .unwrap()
                    };
                    cmd.stdout(f);
                }
                if let Some(stderr) = proc.stderr {
                    let filename = state.pwd.join(stderr);
                    let f = match proc.stderr_mode {
                        RedirMode::Write => File::create(filename).unwrap(),
                        RedirMode::Append => File::options()
                            .append(true)
                            .open(filename)
                            .unwrap()
                    };
                    cmd.stderr(f);
                }

                let _ = cmd
                    .spawn()
                    .expect("")
                    .wait()
                    ;
                state
            } else {
                println!("{}: command not found", proc.exec);
                state
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(a: &[&str]) -> Vec<String> {
        a.iter().map(|a| a.to_string()).collect()
    }

    #[test]
    fn test_words2proc() {
        let argv = args(&["echo", "a", "b"]);
        let result = words2proc(&argv).unwrap();
        assert_eq!(result.exec, "echo");
        assert_eq!(result.argv, vec!["a", "b"]);
        assert_eq!(result.stdout, None);
        assert_eq!(result.stderr, None);

        let argv = args(&["echo", "1", "2"]);
        let result = words2proc(&argv).unwrap();
        assert_eq!(result.exec, "echo");
        assert_eq!(result.argv, vec!["1", "2"]);
        assert_eq!(result.stdout, None);
        assert_eq!(result.stderr, None);

        let argv = args(&["echo", "a", ">", "b"]);
        let result = words2proc(&argv).unwrap();
        assert_eq!(result.exec, "echo");
        assert_eq!(result.argv, vec!["a"]);
        assert_eq!(result.stdout, Some("b"));
        assert_eq!(result.stdout_mode, RedirMode::Write);
        assert_eq!(result.stderr, None);

        let argv = args(&["echo", "a", "2", ">", "b"]);
        let result = words2proc(&argv).unwrap();
        assert_eq!(result.exec, "echo");
        assert_eq!(result.argv, vec!["a"]);
        assert_eq!(result.stdout, None);
        assert_eq!(result.stderr, Some("b"));
        assert_eq!(result.stderr_mode, RedirMode::Write);

        let argv = args(&["echo", "a", ">>", "b"]);
        let result = words2proc(&argv).unwrap();
        assert_eq!(result.exec, "echo");
        assert_eq!(result.argv, vec!["a"]);
        assert_eq!(result.stdout, Some("b"));
        assert_eq!(result.stdout_mode, RedirMode::Append);
        assert_eq!(result.stderr, None);

        let argv = args(&["echo", "a", "2", ">>", "b"]);
        let result = words2proc(&argv).unwrap();
        assert_eq!(result.exec, "echo");
        assert_eq!(result.argv, vec!["a"]);
        assert_eq!(result.stdout, None);
        assert_eq!(result.stderr, Some("b"));
        assert_eq!(result.stderr_mode, RedirMode::Append);

    }
}

