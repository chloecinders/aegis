mod shared;

mod raw_console;
pub use raw_console::*;

mod clipboard;
pub use clipboard::win_get_clipboard_data;

mod win_env;
pub use win_env::win_get_current_system;
pub use win_env::win_get_current_user;
pub use win_env::win_get_environment;
