#[allow(unused_imports)]
use std::io::{self, Write};

fn main() {
    // TODO: check posix exit code
    let mut exit_code: Option<i32> = None;
    let stdin = io::stdin();

    // Wait for user input
    while exit_code.is_none() {
        print!("$ ");
        io::stdout().flush().unwrap();
        let mut input = String::new();
        stdin.read_line(&mut input).unwrap();
        let argv: Vec<&str> = input.split_whitespace().collect();
        if argv.first().filter(|cmd| **cmd == "exit").is_some() {
            let code = argv.get(1).map(|v| v.parse::<i32>()).unwrap_or(Ok(0));
            if let Err(e) = code {
                println!("{}", e);
                continue;
            } else if let Ok(code) = code {
                exit_code = Some(code);
                continue;
            }
        }
        println!("{}: not found", input.trim());
    }
    std::process::exit(exit_code.unwrap());
}
