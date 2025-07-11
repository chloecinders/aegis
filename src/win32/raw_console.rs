use std::{mem::zeroed, os::windows::raw::HANDLE};

use crate::win32::shared::{
    DWORD, GetConsoleMode, GetKeyboardState, GetStdHandle, INPUT_RECORD, ReadConsoleInputW,
    STD_INPUT_HANDLE, SetConsoleMode, ToUnicode, WORD,
};

const ENABLE_ECHO_INPUT: DWORD = 0x4;
const ENABLE_LINE_INPUT: DWORD = 0x2;
const ENABLE_PROCESSED_INPUT: DWORD = 0x1;
const KEY_EVENT: WORD = 0x1;
const RIGHT_ALT_PRESSED: DWORD = 0x1;
const LEFT_ALT_PRESSED: DWORD = 0x2;
const SHIFT_PRESSED: DWORD = 0x10;
const CTRL_PRESSED: DWORD = 0x8;
const VK_SHIFT: DWORD = 0x10;
const VK_CONTROL: DWORD = 0x11;
const VK_MENU: DWORD = 0x12;
const VK_CAPITAL: DWORD = 0x14;

pub enum WRCONRawInputError {
    NoSTDINHandle,
    SetConsoleMode,
}

pub enum WRCONCreateError {
    NoSTDINHandle,
}

pub enum WRCONRawCreateError {
    InputErr(WRCONRawInputError),
    CreateErr(WRCONCreateError),
}

pub enum WRCONReadError {
    FailedConsoleRead,
    InvalidEventType,
}

#[derive(Debug)]
#[allow(unused)]
pub struct ReadEvent {
    pub virtual_key: u16,
    pub character: Option<char>,
    pub ralt_pressed: bool,
    pub lalt_pressed: bool,
    pub shift_pressed: bool,
    pub ctrl_pressed: bool,
}

#[allow(non_camel_case_types)]
/// Raw console input from the win32 API
/// w = windows, r = raw, con = console
pub struct wrcon {
    handle: HANDLE,
}

impl wrcon {
    pub fn new() -> Result<Self, WRCONRawCreateError> {
        if let Err(e) = Self::enable_raw_mode() {
            return Err(WRCONRawCreateError::InputErr(e));
        }

        let handle = unsafe { GetStdHandle(STD_INPUT_HANDLE) };

        if handle.is_null() {
            Err(WRCONRawCreateError::CreateErr(
                WRCONCreateError::NoSTDINHandle,
            ))
        } else {
            Ok(Self { handle: handle })
        }
    }

    fn enable_raw_mode() -> Result<DWORD, WRCONRawInputError> {
        unsafe {
            let h_stdin = GetStdHandle(STD_INPUT_HANDLE);
            if h_stdin.is_null() {
                return Err(WRCONRawInputError::NoSTDINHandle);
            }

            let mut mode: DWORD = 0;
            if GetConsoleMode(h_stdin, &mut mode) == 0 {
                return Err(WRCONRawInputError::SetConsoleMode);
            }

            let raw_mode = mode & !(ENABLE_LINE_INPUT | ENABLE_ECHO_INPUT | ENABLE_PROCESSED_INPUT);

            if SetConsoleMode(h_stdin, raw_mode) == 0 {
                return Err(WRCONRawInputError::SetConsoleMode);
            }

            Ok(mode)
        }
    }

    pub fn read(&mut self) -> Result<ReadEvent, WRCONReadError> {
        unsafe {
            let mut record: INPUT_RECORD = zeroed();
            let mut read = 0;

            if ReadConsoleInputW(self.handle, &mut record, 1, &mut read) == 0 {
                return Err(WRCONReadError::FailedConsoleRead);
            }

            if record.event_type == KEY_EVENT && record.event.bKeyDown != 0 {
                let key_event = record.event;
                let vk = key_event.wVirtualKeyCode;
                let mods = key_event.dwControlKeyState;

                Ok(ReadEvent {
                    virtual_key: vk,
                    character: Self::vkey_to_char(vk, key_event.wVirtualScanCode, mods),
                    ralt_pressed: mods & RIGHT_ALT_PRESSED != 0,
                    lalt_pressed: mods & LEFT_ALT_PRESSED != 0,
                    shift_pressed: mods & SHIFT_PRESSED != 0,
                    ctrl_pressed: mods & CTRL_PRESSED != 0,
                })
            } else {
                Err(WRCONReadError::InvalidEventType)
            }
        }
    }

    fn vkey_to_char(vkey: u16, scancode: u16, mods: u32) -> Option<char> {
        unsafe {
            let key_state = Self::build_keyboard_state_from_flags_only(mods);

            let mut buff = [0u16; 4];
            let result = ToUnicode(
                vkey as u32,
                scancode as u32,
                key_state.as_ptr(),
                buff.as_mut_ptr(),
                buff.len() as i32,
                0,
            );

            if result > 0 {
                Some(std::char::from_u32(buff[0] as u32).unwrap_or('?'))
            } else {
                None
            }
        }
    }

    fn build_keyboard_state_from_flags_only(mods: u32) -> [u8; 256] {
        let mut key_state = [0u8; 256];

        key_state[VK_SHIFT as usize] = if mods & SHIFT_PRESSED != 0 { 0x80 } else { 0 };
        key_state[VK_CONTROL as usize] = if mods & CTRL_PRESSED != 0 { 0x80 } else { 0 };
        key_state[VK_MENU as usize] = if mods & (LEFT_ALT_PRESSED | RIGHT_ALT_PRESSED) != 0 {
            0x80
        } else {
            0
        };

        let capslock_on = unsafe { (GetKeyboardState(VK_CAPITAL as *mut u8) & 1) != 0 };
        key_state[VK_CAPITAL as usize] = if capslock_on { 1 } else { 0 };

        key_state
    }
}
