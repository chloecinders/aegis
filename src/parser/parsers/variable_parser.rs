use std::collections::VecDeque;

use crate::{
    lexer::{Token, TokenStream},
    parser::{AstNode, Expr, ParseError, Parser},
};

pub fn parse_variable_assignment(stream: &mut TokenStream) -> Result<Expr, ParseError> {
    if let Some(Token::Word(name)) = stream.next() {
        stream.expect(Token::Operator(crate::lexer::Operator::Equal))?;

        let mut expr_vec: VecDeque<Token> = VecDeque::new();

        while let Some(token) = stream.peek() {
            if *token == Token::Semicolon {
                break;
            }

            expr_vec.push_back(stream.next().unwrap());
        }

        let inner_expr_stream = TokenStream::new(expr_vec);

        if let Some(Token::Word(_)) = inner_expr_stream.peek() {
            return Err(ParseError::MustBeExpr);
        }

        let expr = {
            if let Ok(AstNode::Expr(expr)) = Parser::parse_stmt_borrow(inner_expr_stream).1 {
                Ok(expr)
            } else {
                Err(ParseError::UnexpectedToken)
            }
        }?;

        stream.expect(Token::Semicolon)?;

        Ok(Expr::VariableAssign {
            name: name,
            value: Box::new(expr),
        })
    } else {
        Err(ParseError::UnexpectedToken)
    }
}
