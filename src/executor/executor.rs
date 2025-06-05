use crate::{
    executor::primitives::traits::{PrimitiveValue, bin_ops::PrimitiveBinOps},
    parser::{AstNode, Expr, Operator, Program},
};

#[derive(Debug)]
pub enum ExecutorError {
    Placeholder,
}

pub struct Executor;

impl Executor {
    pub fn execute(program: Program) -> Result<(), ExecutorError> {
        let mut ast = program.ast.into_iter();

        while let Some(expr) = ast.next() {
            match expr {
                AstNode::Expr(expr) => Self::execute_expression(expr),
                _ => Err(ExecutorError::Placeholder),
            }?;
        }

        println!();
        Ok(())
    }

    fn execute_expression(expr: Expr) -> Result<(), ExecutorError> {
        match expr {
            Expr::Int(i) => print!("{}", i.display()),
            Expr::String(s) => print!("{s}"),
            Expr::Float(f) => print!("{}", f.display()),
            Expr::Bool(b) => print!("{b}"),
            Expr::Operation { .. } => Self::execute_expression(Self::evaluate_expression(expr)?)?,
            _ => return Err(ExecutorError::Placeholder),
        };

        Ok(())
    }

    fn evaluate_expression(expr: Expr) -> Result<Expr, ExecutorError> {
        match expr {
            Expr::Int(_) => Ok(expr),
            Expr::String(s) => Ok(Expr::String(s)),
            Expr::Float(_) => Ok(expr),
            Expr::Bool(_) => Ok(expr),
            Expr::Operation { left, op, right } => Self::evaluate_expr_operation(left, op, right),
            _ => Err(ExecutorError::Placeholder),
        }
    }

    fn evaluate_expr_operation(
        left: Box<Expr>,
        op: Operator,
        right: Box<Expr>,
    ) -> Result<Expr, ExecutorError> {
        let left_eval = Self::evaluate_expression(*left)?;

        let left_trait: &dyn PrimitiveBinOps = match &left_eval {
            Expr::Int(int) => int,
            Expr::Float(f) => f,
            _ => return Err(ExecutorError::Placeholder),
        };

        let right_eval = Self::evaluate_expression(*right)?;

        let right_trait: &dyn PrimitiveBinOps = match &right_eval {
            Expr::Int(i) => i,
            Expr::Float(f) => f,
            _ => return Err(ExecutorError::Placeholder),
        };

        match op {
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
        }
    }
}
