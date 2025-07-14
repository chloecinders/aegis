use std::{ffi::CStr, vec};

use crate::native::{
    environment::Environment,
    win32::shared::{
        DWORD, FreeEnvironmentStringsW, GetComputerNameA, GetEnvironmentStringsW, GetUserNameA,
    },
};

pub fn win_get_current_user() -> Option<String> {
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

pub fn win_get_current_system() -> Option<String> {
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

pub fn win_get_environment() -> Environment {
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
