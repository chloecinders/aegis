use crate::{
    executor::{
        Scope,
        evaluations::{evaluate_command, execute_expression},
    },
    parser::{AstNode, Program},
    win32::Environment,
};

#[derive(Debug)]
pub enum ExecutorError {
    Placeholder,
}

pub struct Executor;

impl Executor {
    pub fn execute(program: Program, environment: &Environment) -> Result<(), ExecutorError> {
        let mut ast = program.ast.into_iter();
        let mut res: String = String::default();
        let mut scope = Scope::new(environment);

        while let Some(expr) = ast.next() {
            res = match expr {
                AstNode::Expr(expr) => execute_expression(&mut scope, AstNode::Expr(expr)),
                AstNode::Cmd(cmd) => evaluate_command(&mut scope, cmd.name, cmd.args),
            }?;
        }

        println!("{res}");
        Ok(())
    }
}
