use std::any::Any;

use crate::executor::primitives::{FloatPrimitive, IntPrimitive};

#[derive(Debug)]
pub struct Program {
    pub ast: Vec<AstNode>,
}

#[derive(Debug)]
pub enum AstNode {
    Expr(Expr),
    Cmd(Cmd),
}

#[derive(Debug)]
pub struct Cmd {
    name: String,
    args: Vec<String>,
}

#[derive(Debug)]
pub enum Expr {
    Int(IntPrimitive),
    Float(FloatPrimitive),
    String(String),
    Bool(bool),
    Variable(String),
    Operation {
        left: Box<Expr>,
        op: Operator,
        right: Box<Expr>,
    },
    If {
        condition: Box<Expr>,
        body: Vec<Box<AstNode>>,
    },
}

impl Expr {
    pub fn as_any(&self) -> &dyn Any {
        self
    }
}

#[derive(Debug)]
pub enum Operator {
    Add,
    Subtract,
    Multiply,
    Divide,
    Modulo,
    Asign,
    Compare,
}
