use std::{cell::RefCell, rc::Rc};

use crate::{
    executor::{
        Scope,
        evaluations::{evaluate_till_primitive, execute_expression},
        executor::ExecutorError,
        expr_is_truthy,
    },
    parser::{AstNode, Expr},
};

pub fn evaluate_while(
    scope: Rc<RefCell<Scope>>,
    cond: Box<Expr>,
    body: Vec<Box<AstNode>>,
) -> Result<Expr, ExecutorError> {
    loop {
        let eval_cond = evaluate_till_primitive(Rc::clone(&scope), *cond.clone())?;

        if !expr_is_truthy(eval_cond) {
            break;
        }

        let mut body_iter = body.clone().into_iter().peekable();
        let new_scope = Scope::from_parent(&scope);
        new_scope.borrow_mut().shadow_scope(&Rc::clone(&scope));

        while let Some(node) = body_iter.next() {
            execute_expression(Rc::clone(&new_scope), *node)?;
        }
    }

    Ok(Expr::None)
}
