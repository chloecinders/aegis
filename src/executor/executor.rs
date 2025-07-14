use std::{cell::RefCell, rc::Rc};

use crate::{
    executor::{
        Scope,
        evaluations::{evaluate_command, execute_expression},
    },
    native::Environment,
    parser::{AstNode, Program},
};

#[derive(Debug)]
pub enum ExecutorError {
    Placeholder,
}

pub struct Executor;

impl Executor {
    pub fn execute(
        program: Program,
        environment: &'static Environment,
    ) -> Result<(), ExecutorError> {
        let mut ast = program.ast.into_iter();
        let mut res: String = String::default();
        let scope = Rc::new(RefCell::new(Scope::new(environment)));

        while let Some(expr) = ast.next() {
            res = match expr {
                AstNode::Expr(expr) => execute_expression(Rc::clone(&scope), AstNode::Expr(expr)),
                AstNode::Cmd(cmd) => evaluate_command(Rc::clone(&scope), cmd.name, cmd.args),
            }?;
        }

        println!("{res}");
        Ok(())
    }
}
