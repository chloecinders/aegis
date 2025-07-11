use std::any::Any;

use crate::{
    executor::primitives::{BoolPrimitive, FloatPrimitive, IntPrimitive, StringPrimitive},
    lexer::Token,
};

#[derive(Debug)]
pub struct Program {
    pub ast: Vec<AstNode>,
}

#[derive(Debug, Clone)]
pub enum AstNode {
    Expr(Expr),
    Cmd(Cmd),
}

#[derive(Debug, Clone)]
pub struct Cmd {
    pub name: String,
    pub args: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct IfCondition {
    pub condition: Box<Expr>,
    pub body: Vec<Box<AstNode>>,
    pub implicit_return: bool,
}

#[derive(Debug, Clone)]
pub enum Expr {
    None,
    Int(IntPrimitive),
    Float(FloatPrimitive),
    String(StringPrimitive),
    Bool(BoolPrimitive),
    Function(),
    Word(String),
    VariableAssign { name: String, value: Box<Expr> },
    Operation { stack: Vec<Box<Token>> },
    If { conditions: Vec<IfCondition> },
}

impl Expr {
    pub fn as_any(&self) -> &dyn Any {
        self
    }
}

#[derive(Debug, Clone)]
pub enum Operator {
    Add,
    Subtract,
    Multiply,
    Divide,
    Modulo,
    Assign,
    Compare,
}
