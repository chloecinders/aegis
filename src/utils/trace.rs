use std::cell::LazyCell;

pub const TRACE_ENV: LazyCell<Vec<String>> = LazyCell::new(|| {
    let env_var = std::env::var("TRACE").unwrap_or(String::default());
    let allowed = vec!["debug", "info", "warn", "error"];

    env_var
        .split(',')
        .filter_map(|s| {
            let trim = s.trim();
            if allowed.contains(&trim) {
                Some(String::from(trim))
            } else {
                None
            }
        })
        .collect()
});

#[macro_export]
macro_rules! debug {
    ($($arg:tt)*) => {
        if (*crate::utils::TRACE_ENV).contains(&String::from("debug")) {
            println!("[DEBUG] {}", format!($($arg)*));
        }
    };
}

#[macro_export]
macro_rules! info {
    ($($arg:tt)*) => {
        if (*crate::utils::TRACE_ENV).contains(&String::from("info")) {
            println!("[INFO] {}", format!($($arg)*));
        }
    };
}

#[macro_export]
macro_rules! warn {
    ($($arg:tt)*) => {
        if (*crate::utils::TRACE_ENV).contains(&String::from("warn")) {
            println!("[WARN] {}", format!($($arg)*));
        }
    };
}

#[macro_export]
macro_rules! error {
    ($($arg:tt)*) => {
        if (*crate::utils::TRACE_ENV).contains(&String::from("error")) {
            println!("[ERROR] {}", format!($($arg)*));
        }
    };
}
