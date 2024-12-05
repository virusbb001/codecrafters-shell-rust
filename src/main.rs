use std::sync::LazyLock;
use std::collections::HashMap;
#[allow(unused_imports)]
use std::io::{self, Write};

type ExitCode = i32;

type BuiltinFunciton = fn(ShellState, &[&str])->ShellState;

struct ShellState {
    exit_code: Option<ExitCode>
}
impl ShellState {
    fn default() -> ShellState {
        ShellState {
            exit_code: None
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

static BUILTIN_FUNCITONS: LazyLock<HashMap<&str, BuiltinFunciton>> = LazyLock::new(|| -> HashMap<&str, BuiltinFunciton> {
    let mut map = HashMap::new();
    map.insert("echo", echo as BuiltinFunciton);
    map.insert("exit", exit as BuiltinFunciton);
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
            } else {
                println!("{}: not found", cmd);
                state
            }
        }
    }
}
