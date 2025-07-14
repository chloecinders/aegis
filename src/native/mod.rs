mod console;
mod environment;
mod system;

#[cfg(target_family = "windows")]
pub mod win32;

pub use system::get_clipboard_data;
pub use system::get_current_system;
pub use system::get_current_user;

pub use environment::Environment;
pub use environment::get_environment;

pub use console::Console;
