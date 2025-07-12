use crate::{
    lexer::{self, Lexer, Token, TokenStream},
    parser::{AstNode, Cmd, Expr, ParseError},
};

pub fn parse_sentence(stream: &mut TokenStream, inner: String) -> Result<AstNode, ParseError> {
    if let Some(Token::Semicolon) = stream.peek() {
        stream.next();
    }

    let mut tokens = match Lexer::parse_strings(inner.as_str()) {
        Ok(t) => t,
        Err(_) => return Err(ParseError::UnexpectedToken),
    };

    let first = match tokens.next() {
        Some(Token::Word(w)) => w,
        Some(Token::String(s)) => s,
        Some(_) => String::default(),
        None => return Err(ParseError::UnexpectedToken),
    };

    if tokens.peek().is_none() {
        Ok(AstNode::Expr(Expr::Word(String::from(first))))
    } else {
        Ok(AstNode::Cmd(Cmd {
            name: String::from(first),
            args: tokens
                .into_iter()
                .map(|t| match t {
                    Token::Word(w) => w,
                    Token::String(s) => s,
                    _ => String::default(),
                })
                .collect(),
        }))
    }
}
