mod if_parser;
pub use if_parser::parse_if_expr;

mod math_parser;
pub use math_parser::parse_math_expr;

mod variable_parser;
pub use variable_parser::parse_variable_assignment;

mod word_parser;
pub use word_parser::parse_sentence;
