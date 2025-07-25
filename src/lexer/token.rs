#[derive(Debug, PartialEq, Clone)]
pub enum TokenKind {
    Word(String),
    Sentence(String),
    String(String),
    Int(i64),
    Float(f64),
    Bool(bool),
    Semicolon,
    Keyword(Keyword),
    OpenBracket,
    ClosedBracket,
    OpenCurlyBracket,
    ClosedCurlyBracket,
    Period,
    Operator(Operator),
}

#[derive(Debug, PartialEq, Clone)]
pub enum Keyword {
    If,
    Let,
    Fn,
    Elseif,
    Else,
    While,
}

#[derive(Debug, PartialEq, Clone)]
pub enum Operator {
    Plus,
    Minus,
    Equal,
    Multiply,
    Divide,
    Modulo,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Token {
    pub kind: TokenKind,
    pub pos: usize,
}
