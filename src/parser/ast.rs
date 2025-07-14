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
    Word(String),
    VariableDeclaration {
        name: String,
        value: Box<Expr>,
    },
    VariableAssigment {
        name: String,
        value: Box<Expr>,
    },
    Operation {
        stack: Vec<Box<Token>>,
    },
    If {
        conditions: Vec<IfCondition>,
    },
    While {
        condition: Box<Expr>,
        body: Vec<Box<AstNode>>,
    },
}

#[derive(Debug, Clone)]
pub enum Operator {
    Add,
    Subtract,
    Multiply,
    Divide,
}
