use std::{cell::RefCell, rc::Rc};

use crate::{executor::executor::ExecutorError, native::Environment, parser::Expr};

#[derive(Debug, Clone)]
pub struct Variable {
    pub name: String,
    pub value: Expr,
}

/// A Scope holding a reference to the current Environment, variables that exist in the current scope and allows current/parent scope overwrite and lookup
pub struct Scope {
    // It's probably okay to have a static reference here as there should never be more than a single environment
    // newly created scopes should use the environment reference passed down from the upper scope
    pub environment: &'static Environment,
    variables: Vec<Variable>,
    shadow: Option<Rc<RefCell<Scope>>>,
}

impl Scope {
    /// Creates a new Scope
    ///
    /// The environment should be reused from the upper scope
    pub fn new(env: &'static Environment) -> Self {
        Self {
            variables: vec![],
            environment: env,
            shadow: None,
        }
    }

    pub fn from_parent(parent_scope: &Rc<RefCell<Scope>>) -> Rc<RefCell<Scope>> {
        let new = Scope::new(parent_scope.borrow().environment);
        Rc::new(RefCell::new(new))
    }

    /// Shadows the parent Scope for stuff like parent variable lookup
    pub fn shadow_scope(&mut self, scope: &Rc<RefCell<Scope>>) {
        self.shadow = Some(Rc::clone(scope))
    }

    /// Pushes a new variable into the Scope
    pub fn push(&mut self, var: Variable) -> Result<(), ExecutorError> {
        if self.variables.iter().any(|v| *v.name == var.name) {
            return Err(ExecutorError::Placeholder);
        }

        self.variables.push(var);
        Ok(())
    }

    /// Overwrites a variables value
    pub fn overwrite(&mut self, name: String, value: Expr) -> Result<(), ExecutorError> {
        if let Some(var) = self.variables.iter_mut().find(|v| v.name == name) {
            var.value = value;
            return Ok(());
        }

        if let Some(ref shadow) = self.shadow {
            return shadow.borrow_mut().overwrite(name, value);
        }

        Err(ExecutorError::Placeholder)
    }

    /// Looks up a variable inside the current and parent Scope
    /// For modifying variables see `overwrite`
    pub fn lookup(&self, var: String) -> Option<Variable> {
        if let Some(v) = self.variables.iter().find(|v| v.name == var) {
            return Some(v.clone());
        }

        if let Some(ref shadow) = self.shadow {
            return shadow.borrow().lookup(var);
        }

        None
    }
}
