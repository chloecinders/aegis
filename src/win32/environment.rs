use std::{
    clone, collections::HashMap, env::current_dir, ffi::CStr, hash::Hash, path::PathBuf, vec,
};

use crate::win32::shared::{
    DWORD, FreeEnvironmentStringsW, GetComputerNameA, GetEnvironmentStringsW, GetUserNameA,
};

pub fn get_current_user() -> Option<String> {
    unsafe {
        let mut buff = [0i8; 256];
        let mut len: DWORD = buff.len() as DWORD;

        if GetUserNameA(buff.as_mut_ptr(), &mut len) == 0 {
            return None;
        }

        let cstr = CStr::from_ptr(buff.as_ptr());
        Some(cstr.to_string_lossy().to_string())
    }
}

pub fn get_current_system() -> Option<String> {
    unsafe {
        let mut buff = [0i8; 256];
        let mut len: DWORD = buff.len() as DWORD;

        if GetComputerNameA(buff.as_mut_ptr(), &mut len) == 0 {
            return None;
        }

        let cstr = CStr::from_ptr(buff.as_ptr());
        Some(cstr.to_string_lossy().to_string())
    }
}

pub fn get_environment_variables() -> Environment {
    unsafe {
        let env_vars = GetEnvironmentStringsW();

        if env_vars.is_null() {
            return Environment::default();
        }

        let mut vars: Vec<String> = vec![];
        let mut ptr = env_vars;

        while *ptr != 0 {
            let mut len = 0;
            while *ptr.add(len) != 0 {
                len += 1;
            }

            let slice = std::slice::from_raw_parts(ptr, len);
            vars.push(String::from_utf16_lossy(slice));

            ptr = ptr.add(len + 1);
        }

        FreeEnvironmentStringsW(env_vars);

        Environment::from(vars)
    }
}

#[derive(Debug)]
pub struct Environment {
    inner: HashMap<String, Vec<String>>,
    directory: PathBuf,
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
        }
    }
}

impl Environment {
    pub fn new(inner: HashMap<String, Vec<String>>) -> Self {
        let directory = match current_dir() {
            Ok(d) => d,
            Err(_) => PathBuf::new(),
        };

        Self { inner, directory }
    }

    pub fn var(&self, name: String) -> Option<&Vec<String>> {
        self.inner.get(&name)
    }

    pub fn find_executable(&self, name: String) -> Option<PathBuf> {
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

        None
    }
}
