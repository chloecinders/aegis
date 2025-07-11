use crate::{
    executor::{
        Scope,
        executor::ExecutorError,
        parse_operator_token,
        primitives::{
            BoolPrimitive, FloatPrimitive, IntPrimitive, StringPrimitive,
            traits::{PrimitiveValue, bin_ops::PrimitiveBinOps},
        },
    },
    lexer::Token,
    parser::{Expr, Operator},
};

pub fn evaluate_math(scope: &mut Scope, expr: Vec<Box<Token>>) -> Result<Expr, ExecutorError> {
    let mut stack: Vec<Expr> = vec![];

    for boxed in expr {
        let token = *boxed;
        match token {
            Token::Word(w) => {
                if let Some(var) = scope.find(w) {
                    stack.push(var.value.clone());
                } else {
                    return Err(ExecutorError::Placeholder);
                }
            }
            Token::String(s) => stack.push(StringPrimitive::from_value_to_expr(s)),
            Token::Int(i) => {
                stack.push(IntPrimitive::from_value_to_expr(i));
            }
            Token::Float(f) => stack.push(FloatPrimitive::from_value_to_expr(f)),
            Token::Bool(b) => stack.push(BoolPrimitive::from_value_to_expr(b)),
            Token::Operator(_) => {
                let Some(right) = stack.pop() else {
                    return Err(ExecutorError::Placeholder);
                };
                let Some(left) = stack.pop() else {
                    return Err(ExecutorError::Placeholder);
                };

                let left_trait: &dyn PrimitiveBinOps = match left {
                    Expr::Int(i) => &i.clone(),
                    Expr::Float(f) => &f.clone(),
                    _ => return Err(ExecutorError::Placeholder),
                };

                let right_trait: &dyn PrimitiveBinOps = match right {
                    Expr::Int(i) => &i.clone(),
                    Expr::Float(f) => &f.clone(),
                    _ => return Err(ExecutorError::Placeholder),
                };

                let op = parse_operator_token(&token)?;

                let res = match op {
                    Operator::Add => match left_trait.bin_add(right_trait) {
                        Some(a) => Ok(a),
                        None => Err(ExecutorError::Placeholder),
                    },
                    Operator::Subtract => match left_trait.bin_sub(right_trait) {
                        Some(a) => Ok(a),
                        None => Err(ExecutorError::Placeholder),
                    },
                    Operator::Multiply => match left_trait.bin_mul(right_trait) {
                        Some(a) => Ok(a),
                        None => Err(ExecutorError::Placeholder),
                    },
                    Operator::Divide => match left_trait.bin_div(right_trait) {
                        Some(a) => Ok(a),
                        None => Err(ExecutorError::Placeholder),
                    },
                    _ => Err(ExecutorError::Placeholder),
                }?;

                stack.push(res);
            }
            _ => {}
        };
    }

    if stack.len() != 1 {
        Err(ExecutorError::Placeholder)
    } else {
        Ok(stack.pop().unwrap())
    }
}
