#![feature(let_chains)]
#![feature(if_let_guard)]
#![feature(string_remove_matches)]

use crate::shell::Shell;

mod executor;
mod lexer;
mod parser;
mod shell;
mod utils;
mod win32;

fn main() {
    let mut shell = Shell::new();
    shell.run();
}
