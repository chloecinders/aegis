use crate::{
    lexer::{Token, TokenStream},
    parser::{AstNode, Cmd, Expr, ParseError},
};

pub fn parse_sentence(stream: &mut TokenStream, inner: String) -> Result<AstNode, ParseError> {
    if let Some(Token::Semicolon) = stream.peek() {
        stream.next();
    }

    let mut split = inner.split_whitespace().peekable();
    let first = split.next().unwrap();

    if split.peek().is_none() {
        Ok(AstNode::Expr(Expr::Word(String::from(first))))
    } else {
        Ok(AstNode::Cmd(Cmd {
            name: String::from(first),
            args: split.map(|s| String::from(s)).collect(),
        }))
    }
}
