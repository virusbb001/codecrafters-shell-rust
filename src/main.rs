#[allow(unused_imports)]
use std::io::{self, Write};

type ExitCode = i32;

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

fn eval(mut state: ShellState, argv: &[&str]) -> ShellState{
    let cmd = argv.first();
    match cmd {
        None => state,
        Some(&"exit") => {
            let code = argv.get(1).map(|v| v.parse::<ExitCode>()).unwrap_or(Ok(0));
            if let Err(e) = code {
                println!("{}", e);
            } else if let Ok(code) = code {
                state.exit_code = Some(code);
            }
            state
        }
        Some(&"echo") => {
            let messages = argv[1..].join(" ");
            println!("{}", messages);
            state
        }
        Some(cmd) => {
            println!("{}: not found", cmd);
            state
        }
    }
}
