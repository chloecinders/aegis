mod expression;
pub use expression::*;

mod mathop;
pub use mathop::evaluate_math;

mod if_statement;
pub use if_statement::evaluate_if;

mod variables;

mod word;
pub use word::evaluate_word;

mod command;
pub use command::evaluate_command;

mod while_statement;
pub use while_statement::evaluate_while;
