use std::{
    env::current_dir,
    fs::File,
    io::{self, Read, Write},
    path::PathBuf,
    process::exit,
    vec,
};

use crate::{
    executor::Executor,
    info,
    lexer::Lexer,
    native::{
        Console, Environment, get_clipboard_data, get_current_system, get_current_user,
        get_environment,
    },
    parser::Parser,
    shell::shell_cmds,
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

            let env = Self::get_env();
            self.exec(env, &contents.as_str());
            return;
        }

        self.consume();
    }

    pub fn consume(&mut self) {
        let env = Self::get_env();

        let mut raw_console = match Console::new() {
            Ok(c) => c,
            Err(_) => {
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
                self.user, self.system, dir,
            );

            io::stdout().flush().unwrap();

            let mut input = String::new();

            loop {
                if let Ok(event) = raw_console.read() {
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

            self.exec(env, input);
        }
    }

    pub fn fallback_loop(&self) {
        let env = Self::get_env();

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

            self.exec(env, input);
        }
    }

    pub fn exec(&self, env: &'static Environment, input: &str) {
        let token_stream = match Lexer::parse(input) {
            Ok(p) => p,
            Err(e) => {
                eprintln!("Error during lexing; err = {e:?}");
                return;
            }
        };

        info!("Token Stream: {token_stream:?}");

        let program = match Parser::parse(token_stream) {
            Ok(p) => p,
            Err(e) => {
                eprintln!("Error during parsing; err = {e:?}");
                return;
            }
        };

        info!("AST: {:#?}", program);

        if let Err(e) = Executor::execute(program, env) {
            eprintln!("Error during execution; err = {e:?}");
        }
    }

    pub fn get_env() -> &'static Environment {
        let env = Box::leak(Box::new(get_environment()));
        env.cmds = vec![(String::from("echo"), Box::new(shell_cmds::echo))];

        env
    }
}
