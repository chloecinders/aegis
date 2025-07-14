use std::{cell::RefCell, rc::Rc};

use crate::{
    executor::{Scope, Variable, evaluations::evaluate_till_primitive, executor::ExecutorError},
    parser::Expr,
};

pub fn evaluate_variable_declaration(
    scope: Rc<RefCell<Scope>>,
    name: String,
    value: Box<Expr>,
) -> Result<Expr, ExecutorError> {
    let evaluated_expr = evaluate_till_primitive(Rc::clone(&scope), *value)?;

    scope.borrow_mut().push(Variable {
        name,
        value: evaluated_expr,
    })?;

    Ok(Expr::None)
}

pub fn evaluate_variable_assignment(
    scope: Rc<RefCell<Scope>>,
    name: String,
    value: Box<Expr>,
) -> Result<Expr, ExecutorError> {
    let evaluated_expr = evaluate_till_primitive(Rc::clone(&scope), *value)?;
    scope.borrow_mut().overwrite(name, evaluated_expr)?;
    Ok(Expr::None)
}
