#![allow(unreachable_code)]

use std::{collections::HashMap, env::current_dir, path::PathBuf};

#[cfg(target_family = "windows")]
use crate::native::win32::win_get_environment;

pub fn get_environment() -> Environment {
    #[cfg(target_family = "windows")]
    return win_get_environment();

    todo!()
}

pub struct Environment {
    inner: HashMap<String, Vec<String>>,
    directory: PathBuf,
    pub cmds: Vec<(String, Box<dyn Fn(Vec<String>) -> ()>)>,
}

impl From<Vec<String>> for Environment {
    fn from(value: Vec<String>) -> Self {
        let mut map: HashMap<String, Vec<String>> = HashMap::with_capacity(value.len());

        for var in value {
            if let Some((name, contents)) = var.split_once('=') {
                let contents_vec: Vec<String> =
                    contents.split(';').map(|s| String::from(s)).collect();

                map.insert(String::from(name), contents_vec);
            }
        }

        Self::new(map)
    }
}

impl Default for Environment {
    fn default() -> Self {
        Self {
            inner: Default::default(),
            directory: Default::default(),
            cmds: vec![],
        }
    }
}

impl Environment {
    pub fn new(inner: HashMap<String, Vec<String>>) -> Self {
        let directory = match current_dir() {
            Ok(d) => d,
            Err(_) => PathBuf::new(),
        };

        Self {
            inner,
            directory,
            cmds: vec![],
        }
    }

    pub fn var(&self, name: String) -> Option<&Vec<String>> {
        self.inner.get(&name)
    }

    pub fn find_executable(&self, name: String) -> Option<PathBuf> {
        #[cfg(target_family = "windows")]
        {
            let executable_extensions = self
                .var(String::from("PATHEXT"))
                .unwrap_or(&vec![
                    String::from("exe"),
                    String::from("bat"),
                    String::from("cmd"),
                ])
                .clone();
            let mut file_path = self.directory.clone();

            for ext in &executable_extensions {
                file_path.push(format!("{}.{}", name, ext));

                if file_path.is_file() {
                    return Some(file_path);
                }

                file_path.pop();
            }

            let Some(env_path) = self.var(String::from("Path")) else {
                return None;
            };
            let env_path_clone = env_path.clone();

            for path in env_path_clone {
                let mut buf = PathBuf::from(path);

                for ext in &executable_extensions {
                    buf.push(format!("{}{}", name, ext));

                    if buf.is_file() {
                        return Some(buf);
                    }

                    buf.pop();
                }
            }

            return None;
        }

        todo!()
    }
}
