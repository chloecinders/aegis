use crate::{
    executor::{
        Scope,
        evaluations::execute_expression,
        executor::ExecutorError,
        primitives::{StringPrimitive, traits::PrimitiveValue},
    },
    parser::{AstNode, Cmd, Expr},
};

pub fn evaluate_word(scope: &mut Scope, word: String) -> Result<Expr, ExecutorError> {
    let cloned = word.clone();
    if let Some(var) = scope.find(word) {
        Ok(var.value.clone())
    } else {
        let cmd = Cmd {
            name: cloned,
            args: Vec::new(),
        };

        match execute_expression(scope, AstNode::Cmd(cmd)) {
            Ok(s) => Ok(StringPrimitive::from_value_to_expr(s)),
            Err(_) => return Err(ExecutorError::Placeholder),
        }
    }
}
