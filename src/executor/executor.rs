use crate::{
    executor::{Scope, evaluations::execute_expression},
    parser::{AstNode, Program},
};

#[derive(Debug)]
pub enum ExecutorError {
    Placeholder,
}

pub struct Executor;

impl Executor {
    pub fn execute(program: Program) -> Result<(), ExecutorError> {
        let mut ast = program.ast.into_iter();
        let mut res: String = String::default();
        let mut scope = Scope::new();

        while let Some(expr) = ast.next() {
            res = match expr {
                AstNode::Expr(expr) => execute_expression(&mut scope, expr),
                _ => Err(ExecutorError::Placeholder),
            }?;
        }

        println!("{res}");
        Ok(())
    }
}
