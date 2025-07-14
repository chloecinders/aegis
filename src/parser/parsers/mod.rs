mod r#if;
pub use r#if::parse_if;

mod math;
pub use math::parse_math;

mod variable;
pub use variable::parse_variable_assignment;

mod word;
pub use word::parse_word;

mod sentence;
pub use sentence::parse_sentence;

mod r#while;
pub use r#while::parse_while;

mod body;
pub use body::parse_body;
