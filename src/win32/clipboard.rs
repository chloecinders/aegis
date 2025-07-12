use crate::win32::shared::{
    CloseClipboard, DWORD, GetClipboardData, GlobalLock, GlobalUnlock, OpenClipboard,
};

const CF_UNICODE_TEXT: DWORD = 13;

pub fn get_clipboard_data() -> Option<String> {
    unsafe {
        if OpenClipboard(std::ptr::null_mut()) == 0 {
            return None;
        }

        let data = GetClipboardData(CF_UNICODE_TEXT);

        if data.is_null() {
            CloseClipboard();
            return None;
        }

        let ptr = GlobalLock(data) as *const u16;

        if ptr.is_null() {
            CloseClipboard();
            return None;
        }

        let mut len = 0;

        while *ptr.add(len) != 0 {
            len += 1;
        }

        let slice: &[u16] = std::slice::from_raw_parts(ptr, len);
        let string = String::from_utf16_lossy(slice);

        println!("TEST {string}");

        GlobalUnlock(data);
        CloseClipboard();

        Some(string.to_string())
    }
}
