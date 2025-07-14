use crate::{
    lexer::{Operator, Token, TokenKind, TokenStream},
    parser::{AstNode, Expr, ParseError, Parser},
};

pub fn parse_word(stream: &mut TokenStream, inner: String) -> Result<AstNode, ParseError> {
    if let Some(Token {
        kind: TokenKind::Semicolon,
        ..
    }) = stream.peek()
    {
        stream.next();
    }

    if let Some(Token {
        kind: TokenKind::Operator(Operator::Equal),
        ..
    }) = stream.peek()
    {
        stream.expect(TokenKind::Operator(Operator::Equal))?;

        let node = Parser::parse_single(stream)?;

        if let AstNode::Expr(e) = node {
            Ok(AstNode::Expr(Expr::VariableAssigment {
                name: inner,
                value: Box::new(e),
            }))
        } else {
            Err(ParseError::UnexpectedToken)
        }
    } else {
        Ok(AstNode::Expr(Expr::Word(inner)))
    }
}
