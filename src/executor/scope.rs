use crate::{parser::Expr, win32::Environment};

#[derive(Debug)]
pub struct Variable {
    pub name: String,
    pub value: Expr,
}

#[derive(Debug)]
pub struct Scope<'a> {
    pub environment: &'a Environment,
    variables: Vec<Box<Variable>>,
}

impl<'a> Scope<'a> {
    pub fn new(env: &'a Environment) -> Self {
        Self {
            variables: vec![],
            environment: env,
        }
    }

    pub fn push(&mut self, var: Variable) -> &Self {
        self.variables.push(Box::new(var));
        self
    }

    pub fn find(&self, var: String) -> Option<&Box<Variable>> {
        self.variables.iter().find(|v| v.name == var)
    }
}
