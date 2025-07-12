use crate::{
    executor::{
        Scope,
        evaluations::{
            evaluate_command, evaluate_if_statement, evaluate_math, evaluate_word,
            variables::evaluate_variable_assignment,
        },
        executor::ExecutorError,
        primitives::traits::PrimitiveValue,
    },
    parser::{AstNode, Cmd, Expr},
};

pub fn execute_expression(scope: &mut Scope, node: AstNode) -> Result<String, ExecutorError> {
    let res: Option<String> = match node {
        AstNode::Expr(Expr::None) => Some(String::default()),
        AstNode::Expr(Expr::Int(i)) => Some(i.display()),
        AstNode::Expr(Expr::String(s)) => Some(s.display()),
        AstNode::Expr(Expr::Float(f)) => Some(f.display()),
        AstNode::Expr(Expr::Bool(b)) => Some(b.display()),
        AstNode::Expr(expr) => {
            let evaluted = evaluate_expression(scope, expr)?;
            Some(execute_expression(scope, AstNode::Expr(evaluted))?)
        }
        AstNode::Cmd(Cmd { name, args }) => Some(evaluate_command(scope, name, args)?),
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
            let evaluted = evaluate_expression(scope, expr)?;
            Ok(evaluate_till_primitive(scope, evaluted)?)
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
