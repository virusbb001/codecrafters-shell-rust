use std::env;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::{fs, sync::LazyLock};
use std::collections::HashMap;
use std::process::Command;
#[allow(unused_imports)]
use std::io::{self, Write};

type ExitCode = i32;

type BuiltinFunciton = fn(ShellState, &[&str])->ShellState;

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

fn echo(state: ShellState, argv: &[&str]) -> ShellState {
    let messages = argv.join(" ");
    println!("{}", messages);
    state
}

fn exit(mut state: ShellState, argv: &[&str]) -> ShellState {
    let code = argv.first().map(|v| v.parse::<ExitCode>()).unwrap_or(Ok(0));
    if let Err(e) = code {
        println!("{}", e);
    } else if let Ok(code) = code {
        state.exit_code = Some(code);
    }
    state
}

fn type_fn(state: ShellState, argv: &[&str]) -> ShellState {
    let Some(cmd) = argv.first() else {
        println!("type [cmd]");
        return state
    };
    if BUILTIN_FUNCITONS.get(cmd).is_some() {
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

fn which(state: ShellState, argv: &[&str]) -> ShellState {
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

fn pwd(state: ShellState, _argv: &[&str]) -> ShellState {
    println!("{}", state.pwd.display());
    state
}

fn cd(mut state: ShellState, argv: &[&str]) -> ShellState {
    let new_wd = match argv.first() {
        None => {
            env::home_dir()
        }
        Some(dir) => {
            Some(PathBuf::from(dir))
        }
    };
    let Some(new_wd) = new_wd else {
        println!("failed to get new directory");
        return state;
    };

    match fs::canonicalize(state.pwd.join(new_wd)) {
        Ok(path) => {
            state.pwd = path;
        },
        Err(e) => {
            println!("{}", e);
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
        let argv: Vec<&str> = input.split_whitespace().collect();
        state = eval(state, &argv);
    }
    std::process::exit(state.exit_code.unwrap());
}

fn eval(state: ShellState, argv: &[&str]) -> ShellState{
    let cmd = argv.first();
    match cmd {
        None => state,
        Some(cmd) => {
            if let Some(builtin_fn) = BUILTIN_FUNCITONS.get(cmd) {
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
                println!("{}: not found", cmd);
                state
            }
        }
    }
}
