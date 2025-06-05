#![feature(let_chains)]
#![feature(if_let_guard)]
#![feature(string_remove_matches)]

use std::io::{self, Write};

use lexer::Lexer;
use parser::Parser;

use crate::executor::Executor;

mod executor;
mod lexer;
mod parser;
mod shell;

fn main() {
    loop {
        print!("> ");

        io::stdout().flush().unwrap();

        let mut input = String::new();

        if let Err(e) = io::stdin().read_line(&mut input) {
            eprintln!("Error reading input; err = {:?}", e);
            continue;
        }

        let input = input.trim();

        if input.is_empty() {
            println!("");
            continue;
        }

        if input == "exit" {
            break;
        }

        let token_stream = match Lexer::parse(input) {
            Ok(p) => p,
            Err(e) => {
                eprintln!("Error during lexing; err = {e:?}");
                continue;
            }
        };

        println!("Token Stream: {token_stream:?}");

        let program = match Parser::parse(token_stream) {
            Ok(p) => p,
            Err(e) => {
                eprintln!("Error during parsing; err = {e:?}");
                continue;
            }
        };

        println!("AST: {:?}", program);

        if let Err(e) = Executor::execute(program) {
            eprintln!("Error during execution; err = {e:?}");
        }
    }
}
