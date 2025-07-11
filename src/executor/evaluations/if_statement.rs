use crate::{
    executor::{Scope, evaluations::evaluate_expression, executor::ExecutorError, expr_is_truthy},
    parser::{AstNode, Expr, IfCondition},
};

pub fn evaluate_if_statement(
    scope: &mut Scope,
    conditions: Vec<IfCondition>,
) -> Result<Option<Expr>, ExecutorError> {
    for cond in conditions {
        let eval_cond = evaluate_expression(scope, *cond.condition)?;

        if !expr_is_truthy(eval_cond) {
            continue;
        }

        let mut body_iter = cond.body.into_iter().peekable();

        while let Some(node) = body_iter.next() {
            let expr = match *node {
                AstNode::Expr(e) => e,
                AstNode::Cmd(_) => continue,
            };

            if cond.implicit_return && body_iter.peek().is_none() {
                return Ok(Some(evaluate_expression(scope, expr)?));
            }

            evaluate_expression(scope, expr)?;
        }

        return Ok(None);
    }

    Ok(None)
}
