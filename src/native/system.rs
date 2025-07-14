#![allow(unreachable_code)]

#[cfg(target_family = "windows")]
use crate::native::win32::{win_get_clipboard_data, win_get_current_system, win_get_current_user};

pub fn get_current_user() -> Option<String> {
    #[cfg(target_family = "windows")]
    return win_get_current_user();

    todo!()
}

pub fn get_current_system() -> Option<String> {
    #[cfg(target_family = "windows")]
    return win_get_current_system();

    todo!()
}

pub fn get_clipboard_data() -> Option<String> {
    #[cfg(target_family = "windows")]
    return win_get_clipboard_data();

    todo!()
}
