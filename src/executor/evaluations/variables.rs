use crate::{
    executor::{Scope, Variable, evaluations::evaluate_till_primitive, executor::ExecutorError},
    parser::Expr,
};

pub fn evaluate_variable_assignment(
    scope: &mut Scope,
    name: String,
    value: Box<Expr>,
) -> Result<Expr, ExecutorError> {
    let evaluated_expr = evaluate_till_primitive(scope, *value)?;

    scope.push(Variable {
        name,
        value: evaluated_expr,
    });

    Ok(Expr::None)
}
