use std::{
    ffi::{c_int, c_uint, c_void},
    os::windows::raw::HANDLE,
};

pub type BOOL = i32;
pub type DWORD = u32;
pub type WORD = u16;
pub type WCHAR = u16;
pub type BYTE = u8;
pub type STR = i8;
pub type WSTR = u16;
pub type HWND = *mut c_void;

pub const STD_INPUT_HANDLE: DWORD = -10i32 as DWORD;

#[repr(C)]
#[derive(Clone, Copy)]
#[allow(non_snake_case, non_camel_case_types)]
pub struct KEY_EVENT_RECORD {
    pub bKeyDown: i32,
    pub wRepeatCount: WORD,
    pub wVirtualKeyCode: WORD,
    pub wVirtualScanCode: WORD,
    pub uChar: CHAR_UNION,
    pub dwControlKeyState: DWORD,
}

#[repr(C)]
#[derive(Clone, Copy)]
#[allow(non_snake_case, non_camel_case_types)]
pub struct INPUT_RECORD {
    pub event_type: WORD,
    pub event: KEY_EVENT_RECORD,
}

#[repr(C)]
#[derive(Clone, Copy)]
#[allow(non_snake_case, non_camel_case_types)]
pub union CHAR_UNION {
    UnicodeChar: WCHAR,
    AsciiChar: u8,
}

#[link(name = "advapi32")]
unsafe extern "system" {
    pub fn GetUserNameA(lpBuffer: *mut STR, pcbBuffer: *mut DWORD) -> BOOL;
}

#[link(name = "kernel32")]
unsafe extern "system" {
    pub fn GetStdHandle(nStdHandle: DWORD) -> HANDLE;
    pub fn GetConsoleMode(hConsoleHandle: HANDLE, lpMode: *mut DWORD) -> BOOL;
    pub fn SetConsoleMode(hConsoleHandle: HANDLE, dwMode: DWORD) -> BOOL;
    pub fn ReadConsoleInputW(
        hConsoleInput: HANDLE,
        lpBuffer: *mut INPUT_RECORD,
        nLength: DWORD,
        lpNumberOfEventsRead: *mut DWORD,
    ) -> BOOL;
    pub fn GlobalLock(hMem: HANDLE) -> *mut c_void;
    pub fn GlobalUnlock(hMem: HANDLE) -> BOOL;
    pub fn GetComputerNameA(lpBuffer: *mut STR, nSize: *mut DWORD) -> BOOL;
    pub fn GetEnvironmentStringsW() -> *const WSTR;
    pub fn FreeEnvironmentStringsW(penv: *const WSTR) -> BOOL;
}

#[link(name = "user32")]
unsafe extern "system" {
    pub fn ToUnicode(
        wVirtKey: u32,
        wScanCode: u32,
        lpKeyState: *const BYTE,
        pwszBuff: *mut WCHAR,
        cchBuff: c_int,
        wFlags: u32,
    ) -> c_int;
    pub fn GetKeyboardState(lpKeyState: *mut BYTE) -> i32;
    pub fn OpenClipboard(hWndNewOwner: *mut HWND) -> BOOL;
    pub fn CloseClipboard() -> BOOL;
    pub fn GetClipboardData(uFormat: c_uint) -> HANDLE;
}
