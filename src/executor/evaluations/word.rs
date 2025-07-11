use crate::{
    executor::{Scope, executor::ExecutorError},
    parser::Expr,
};

pub fn evaluate_word(scope: &mut Scope, word: String) -> Result<Expr, ExecutorError> {
    if let Some(var) = scope.find(word) {
        Ok(var.value.clone())
    } else {
        Err(ExecutorError::Placeholder)
    }
}
