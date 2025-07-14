use std::collections::VecDeque;

use crate::{
    lexer::{Token, TokenKind, TokenStream},
    parser::{AstNode, ParseError, Parser},
};

/// Also returns a bool if there is an implicit return inside the body
pub fn parse_body(stream: &mut TokenStream) -> Result<(Vec<AstNode>, bool), ParseError> {
    if let Some(Token {
        kind: TokenKind::OpenBracket,
        ..
    }) = stream.peek()
    {
        stream.next();
    }

    let mut body_count = 1;
    let mut expressions: VecDeque<Token> = VecDeque::new();

    while let Some(t) = stream.next() {
        match t.kind {
            TokenKind::OpenCurlyBracket => body_count += 1,
            TokenKind::ClosedCurlyBracket => {
                body_count -= 1;

                if body_count == 0 {
                    break;
                }
            }
            _ => expressions.push_back(t),
        }
    }

    let implicit_return = expressions.iter().nth_back(0).is_some_and(|t| {
        !matches!(
            t,
            Token {
                kind: TokenKind::Semicolon,
                ..
            }
        )
    });

    let mut stream = TokenStream::new(expressions);
    Ok((Parser::parse_stream(&mut stream)?, implicit_return))
}
