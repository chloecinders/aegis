use crate::{
    lexer::{Token, TokenKind, TokenStream},
    parser::{AstNode, Expr, ParseError, Parser},
};

pub fn parse_variable_assignment(stream: &mut TokenStream) -> Result<Expr, ParseError> {
    if let Some(Token {
        kind: TokenKind::Word(name),
        ..
    }) = stream.next()
    {
        stream.expect(TokenKind::Operator(crate::lexer::Operator::Equal))?;

        let node = Parser::parse_single(stream)?;

        if let AstNode::Expr(expr) = node {
            stream.expect(TokenKind::Semicolon)?;

            Ok(Expr::VariableDeclaration {
                name: name,
                value: Box::new(expr),
            })
        } else {
            Err(ParseError::UnexpectedToken)
        }
    } else {
        Err(ParseError::UnexpectedToken)
    }
}
