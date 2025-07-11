use crate::parser::Expr;

#[derive(Debug)]
pub struct Variable {
    pub name: String,
    pub value: Expr,
}

#[derive(Debug)]
pub struct Scope {
    variables: Vec<Box<Variable>>,
}

impl Scope {
    pub fn new() -> Self {
        Self { variables: vec![] }
    }

    pub fn push(&mut self, var: Variable) -> &Self {
        self.variables.push(Box::new(var));
        self
    }

    pub fn find(&self, var: String) -> Option<&Box<Variable>> {
        self.variables.iter().find(|v| v.name == var)
    }
}
