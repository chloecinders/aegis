use std::{cell::RefCell, rc::Rc};

use crate::{
    executor::{
        Scope,
        evaluations::{evaluate_expression, evaluate_till_primitive},
        executor::ExecutorError,
        expr_is_truthy,
    },
    parser::{AstNode, Expr, IfCondition},
};

pub fn evaluate_if(
    scope: Rc<RefCell<Scope>>,
    conditions: Vec<IfCondition>,
) -> Result<Option<Expr>, ExecutorError> {
    for cond in conditions {
        let eval_cond = evaluate_till_primitive(Rc::clone(&scope), *cond.condition)?;

        if !expr_is_truthy(eval_cond) {
            continue;
        }

        let mut body_iter = cond.body.into_iter().peekable();
        let new_scope = Scope::from_parent(&scope);
        new_scope.borrow_mut().shadow_scope(&Rc::clone(&scope));

        while let Some(node) = body_iter.next() {
            let expr = match *node {
                AstNode::Expr(e) => e,
                AstNode::Cmd(_) => continue,
            };

            if cond.implicit_return && body_iter.peek().is_none() {
                let res = Some(evaluate_expression(Rc::clone(&new_scope), expr)?);
                return Ok(res);
            }

            evaluate_expression(Rc::clone(&new_scope), expr)?;
        }

        return Ok(None);
    }

    Ok(None)
}
