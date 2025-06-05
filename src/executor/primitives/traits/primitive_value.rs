use crate::parser::Expr;

pub trait PrimitiveValue<T> {
    fn display(&self) -> String;
    fn from_value_to_expr(val: T) -> Expr;
}

