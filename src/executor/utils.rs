use crate::executor::executor::ExecutorError;
use crate::parser::Expr;
use crate::{
    lexer::{self, TokenKind},
    parser::Operator,
};

pub fn expr_is_truthy(expr: Expr) -> bool {
    match expr {
        Expr::Int(i) => i.value != 0,
        Expr::Bool(b) => b.value,
        Expr::Float(f) => f.value != 0.0,
        Expr::String(s) => s.value.as_str() != "",
        _ => false,
    }
}

pub fn parse_operator_token(token: &TokenKind) -> Result<Operator, ExecutorError> {
    match token {
        TokenKind::Operator(lexer::Operator::Plus) => Ok(Operator::Add),
        TokenKind::Operator(lexer::Operator::Minus) => Ok(Operator::Subtract),
        TokenKind::Operator(lexer::Operator::Divide) => Ok(Operator::Divide),
        TokenKind::Operator(lexer::Operator::Multiply) => Ok(Operator::Multiply),
        _ => Err(ExecutorError::Placeholder),
    }
}
