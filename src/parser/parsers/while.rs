use crate::{
    lexer::TokenStream,
    parser::{AstNode, Expr, ParseError, Parser, parsers::parse_body},
};

pub fn parse_while(stream: &mut TokenStream) -> Result<Expr, ParseError> {
    let cond = Parser::parse_single(stream)?;
    let body = parse_body(stream)?.0;

    if let AstNode::Expr(cond) = cond {
        Ok(Expr::While {
            condition: Box::new(cond),
            body: body.into_iter().map(|n| Box::new(n)).collect(),
        })
    } else {
        Err(ParseError::UnexpectedToken)
    }
}
