use crate::{
    lexer::{Lexer, Token, TokenKind},
    parser::{AstNode, Cmd, ParseError},
};

pub fn parse_sentence(inner: String) -> Result<AstNode, ParseError> {
    let mut tokens = match Lexer::parse_strings(inner.as_str()) {
        Ok(t) => t,
        Err(_) => return Err(ParseError::UnexpectedToken),
    };

    let first = match tokens.next() {
        Some(Token {
            kind: TokenKind::Word(w),
            ..
        }) => w,
        Some(Token {
            kind: TokenKind::String(s),
            ..
        }) => s,
        Some(_) => String::default(),
        None => return Err(ParseError::UnexpectedToken),
    };

    Ok(AstNode::Cmd(Cmd {
        name: String::from(first),
        args: tokens
            .into_iter()
            .map(|t| match t.kind {
                TokenKind::Word(w) => w,
                TokenKind::String(s) => s,
                _ => String::default(),
            })
            .collect(),
    }))
}
