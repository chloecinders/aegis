use crate::{
    executor::{
        Scope,
        evaluations::{
            evaluate_if_statement, evaluate_math, evaluate_word,
            variables::evaluate_variable_assignment,
        },
        executor::ExecutorError,
        primitives::traits::PrimitiveValue,
    },
    parser::Expr,
};

pub fn execute_expression(scope: &mut Scope, expr: Expr) -> Result<String, ExecutorError> {
    let res: Option<String> = match expr {
        Expr::None => Some(String::default()),
        Expr::Int(i) => Some(i.display()),
        Expr::String(s) => Some(s.display()),
        Expr::Float(f) => Some(f.display()),
        Expr::Bool(b) => Some(b.display()),
        _ => {
            let evaluted = evaluate_expression(scope, expr)?;
            Some(execute_expression(scope, evaluted)?)
        }
    };

    Ok(res.unwrap_or(String::default()))
}

pub fn evaluate_till_primitive(scope: &mut Scope, expr: Expr) -> Result<Expr, ExecutorError> {
    match expr {
        Expr::None => Ok(expr),
        Expr::Int(_) => Ok(expr),
        Expr::String(_) => Ok(expr),
        Expr::Float(_) => Ok(expr),
        Expr::Bool(_) => Ok(expr),
        _ => {
            let evaluted = evaluate_till_primitive(scope, expr)?;
            Ok(evaluted)
        }
    }
}

pub fn evaluate_expression(scope: &mut Scope, expr: Expr) -> Result<Expr, ExecutorError> {
    match expr {
        Expr::Int(_) => Ok(expr),
        Expr::String(s) => Ok(Expr::String(s)),
        Expr::Float(_) => Ok(expr),
        Expr::Bool(_) => Ok(expr),
        Expr::Operation { stack } => match evaluate_math(scope, stack) {
            Ok(e) => Ok(e),
            Err(e) => Err(e),
        },
        Expr::If { conditions } => match evaluate_if_statement(scope, conditions) {
            Ok(Some(e)) => Ok(e),
            Ok(None) => Ok(Expr::None),
            Err(e) => Err(e),
        },
        Expr::VariableAssign { name, value } => {
            Ok(evaluate_variable_assignment(scope, name, value)?)
        }
        Expr::Word(w) => Ok(evaluate_word(scope, w)?),
        _ => Err(ExecutorError::Placeholder),
    }
}
