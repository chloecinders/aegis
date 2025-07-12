use std::{
    collections::HashMap,
    env::current_dir,
    ffi::OsStr,
    fs::File,
    io::{self, Read, Write},
    path::PathBuf,
    process::exit,
};

use crate::{
    debug,
    executor::Executor,
    lexer::Lexer,
    parser::Parser,
    win32::{
        Environment,
        WRCONRawCreateError::{CreateErr, InputErr},
        get_clipboard_data, get_current_system, get_current_user, get_environment_variables, wrcon,
    },
};

pub struct Shell {
    directory: PathBuf,
    user: String,
    system: String,
}

impl Shell {
    pub fn new() -> Self {
        let directory = match current_dir() {
            Ok(d) => d,
            Err(_) => {
                eprintln!("Couldnt get current directory!");
                PathBuf::default()
            }
        };

        let user = get_current_user().unwrap_or(String::default());
        let system = get_current_system().unwrap_or(String::default());

        Self {
            directory,
            user,
            system,
        }
    }

    pub fn run(&mut self) {
        let mut args = std::env::args();
        args.next();

        if let Some(script) = args.next() {
            let working_dir = current_dir().unwrap_or_else(|_| {
                eprintln!("Invalid working directory.");
                exit(1);
            });

            let script_path = working_dir.join(script);

            if !script_path.is_file() {
                eprintln!("Provided script is a directory or does not exist.");
                exit(1);
            }

            let mut file = match File::open(script_path) {
                Ok(f) => f,
                Err(e) => {
                    eprintln!("Could not open provided script; Err = {e:?}");
                    exit(1);
                }
            };

            let mut contents = String::new();

            if let Err(e) = file.read_to_string(&mut contents) {
                eprintln!("Could not read file; Err = {e:?}");
                exit(1);
            }

            let env = get_environment_variables();
            self.exec(&env, &contents.as_str());
            return;
        }

        self.consume();
    }

    pub fn consume(&self) {
        let env = get_environment_variables();

        let mut wrcon = match wrcon::new() {
            Ok(w) => w,
            Err(e) => {
                match e {
                    InputErr(_) => println!("Could not set raw input mode."),
                    CreateErr(_) => println!("Could not obtain STDIN handle."),
                };
                println!(
                    "Special control sequences such as CTRL + C may not work/have unintended behavior!"
                );

                self.fallback_loop();
                exit(0)
            }
        };

        'outer: loop {
            let dir = self
                .directory
                .iter()
                .map(|s| format!(" {}", s.to_string_lossy().to_string()))
                .last()
                .unwrap_or(String::default());

            print!(
                "\x1b[38;5;212m{}\x1b[0m@\x1b[38;5;186m{}\x1b[38;5;152m{}\x1b[0m A>",
                self.user, self.system, dir
            );

            io::stdout().flush().unwrap();

            let mut input = String::new();

            loop {
                if let Ok(event) = wrcon.read() {
                    match event.virtual_key {
                        0 => continue,
                        // Enter
                        0x0D => {
                            if event.shift_pressed {
                                input.push_str("\r\n");
                                print!("\r\n");
                            } else {
                                break;
                            }
                        }
                        // Ctrl + V
                        0x16 if event.ctrl_pressed => {
                            if let Some(data) = get_clipboard_data() {
                                input.push_str(data.as_str());
                                print!("{data}");
                            }
                        }
                        // Ctrl + D
                        0x44 if event.ctrl_pressed => {
                            break 'outer;
                        }
                        // Ctrl + C
                        0x03 => {
                            println!("");
                            continue 'outer;
                        }
                        0x1b => {
                            println!("test");
                        }
                        // Backspace
                        0x08 => {
                            if !input.is_empty() {
                                input.pop();
                                print!("\x08 \x08");
                                io::stdout().flush().unwrap()
                            }
                        }
                        _ => {
                            if let Some(char) = event.character {
                                input.push(char);
                                print!("{char}");
                                io::stdout().flush().unwrap();
                            }
                        }
                    }
                }
            }

            let input = input.trim();

            println!("");
            io::stdout().flush().unwrap();

            if input == "exit" {
                break;
            }

            self.exec(&env, input);
        }
    }

    pub fn fallback_loop(&self) {
        let env = Box::new(get_environment_variables());

        loop {
            print!(" > ");

            io::stdout().flush().unwrap();

            let mut input = String::new();

            if let Err(e) = io::stdin().read_line(&mut input) {
                eprintln!("Error reading input; err = {:?}", e);
                continue;
            }

            let input = input.trim();

            if input.is_empty() {
                continue;
            }

            if input == "exit" {
                break;
            }

            self.exec(&env, input);
        }
    }

    pub fn exec(&self, env: &Environment, input: &str) {
        let token_stream = match Lexer::parse(input) {
            Ok(p) => p,
            Err(e) => {
                eprintln!("Error during lexing; err = {e:?}");
                return;
            }
        };

        debug!("Token Stream: {token_stream:?}");

        let program = match Parser::parse(token_stream) {
            Ok(p) => p,
            Err(e) => {
                eprintln!("Error during parsing; err = {e:?}");
                return;
            }
        };

        debug!("AST: {:?}", program);

        if let Err(e) = Executor::execute(program, env) {
            eprintln!("Error during execution; err = {e:?}");
        }
    }
}
