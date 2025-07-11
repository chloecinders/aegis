use std::collections::VecDeque;

use crate::{
    executor::primitives::{BoolPrimitive, traits::PrimitiveValue},
    lexer::{Keyword, Token, TokenStream},
    parser::{AstNode, Expr, IfCondition, ParseError, Parser},
};

pub fn parse_if_expr(stream: &mut TokenStream) -> Result<Expr, ParseError> {
    fn consume(inner_stream: &mut TokenStream) -> Result<IfCondition, ParseError> {
        let mut cond_stream: Option<TokenStream> = None;
        let is_else = inner_stream
            .peek()
            .is_some_and(|t| matches!(t, &Token::OpenCurlyBracket));

        if !is_else {
            let mut cond_vec: VecDeque<Token> = VecDeque::new();

            while let Some(token) = inner_stream.peek() {
                if *token == Token::OpenCurlyBracket {
                    break;
                }

                cond_vec.push_back(inner_stream.next().unwrap());
            }

            let inner_cond_stream = TokenStream::new(cond_vec);

            if let Some(Token::Word(_)) = inner_cond_stream.peek() {
                return Err(ParseError::MustBeExpr);
            }

            cond_stream = Some(inner_cond_stream);
        }

        inner_stream.expect(Token::OpenCurlyBracket)?;

        let mut body_vec: VecDeque<Token> = VecDeque::new();

        while let Some(token) = inner_stream.peek() {
            if *token == Token::ClosedCurlyBracket {
                break;
            }

            body_vec.push_back(inner_stream.next().unwrap());
        }

        inner_stream.expect(Token::ClosedCurlyBracket)?;

        let is_last_return = body_vec
            .iter()
            .nth_back(0)
            .is_some_and(|t| !matches!(t, Token::Semicolon));
        let program = Parser::parse(TokenStream::new(body_vec))?;
        let statement_body: Vec<Box<AstNode>> =
            program.ast.into_iter().map(|expr| Box::new(expr)).collect();

        if is_else {
            Ok(IfCondition {
                condition: Box::new(Expr::Bool(BoolPrimitive::new(true))),
                body: statement_body,
                implicit_return: is_last_return,
            })
        } else {
            let cond_expr = {
                if let Ok(AstNode::Expr(expr)) = Parser::parse_stmt_borrow(cond_stream.unwrap()).1 {
                    Ok(expr)
                } else {
                    Err(ParseError::UnexpectedToken)
                }
            }?;

            Ok(IfCondition {
                condition: Box::new(cond_expr),
                body: statement_body,
                implicit_return: is_last_return,
            })
        }
    }

    let mut conditions: Vec<IfCondition> = vec![];
    conditions.push(consume(stream)?);

    while let Some(next_token) = stream.next() {
        if matches!(next_token, Token::Keyword(Keyword::Elseif)) {
            conditions.push(consume(stream)?);
        } else if matches!(next_token, Token::Keyword(Keyword::Else)) {
            conditions.push(consume(stream)?);
            break;
        }
    }

    Ok(Expr::If {
        conditions: conditions,
    })
}
