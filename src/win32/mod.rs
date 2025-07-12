mod shared;

mod raw_console;
pub use raw_console::*;

mod clipboard;
pub use clipboard::get_clipboard_data;

mod environment;
pub use environment::Environment;
pub use environment::get_current_system;
pub use environment::get_current_user;
pub use environment::get_environment_variables;
