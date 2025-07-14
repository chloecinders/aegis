use std::collections::VecDeque;

use crate::{
    executor::primitives::{BoolPrimitive, traits::PrimitiveValue},
    lexer::{Keyword, Token, TokenKind, TokenStream},
    parser::{AstNode, Expr, IfCondition, ParseError, Parser, parsers::parse_body},
};

pub fn parse_if(stream: &mut TokenStream) -> Result<Expr, ParseError> {
    fn consume(inner_stream: &mut TokenStream) -> Result<IfCondition, ParseError> {
        let mut cond_stream: Option<TokenStream> = None;
        let is_else = inner_stream
            .peek()
            .is_some_and(|t| matches!(t.kind, TokenKind::OpenCurlyBracket));

        if !is_else {
            let mut cond_vec: VecDeque<Token> = VecDeque::new();

            while let Some(token) = inner_stream.peek() {
                if (*token).kind == TokenKind::OpenCurlyBracket {
                    break;
                }

                cond_vec.push_back(inner_stream.next().unwrap());
            }

            let inner_cond_stream = TokenStream::new(cond_vec);

            if let Some(Token {
                kind: TokenKind::Word(_),
                ..
            }) = inner_cond_stream.peek()
            {
                return Err(ParseError::MustBeExpr);
            }

            cond_stream = Some(inner_cond_stream);
        }

        inner_stream.expect(TokenKind::OpenCurlyBracket)?;

        let (statement_body, is_last_return) = parse_body(inner_stream)?;
        let statement_body: Vec<Box<AstNode>> =
            statement_body.into_iter().map(|n| Box::new(n)).collect();

        if is_else {
            Ok(IfCondition {
                condition: Box::new(Expr::Bool(BoolPrimitive::new(true))),
                body: statement_body,
                implicit_return: is_last_return,
            })
        } else {
            let cond_expr = {
                if let Ok(AstNode::Expr(expr)) = Parser::parse_single(&mut cond_stream.unwrap()) {
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

    while let Some(next_token) = stream.peek() {
        if matches!(next_token.kind, TokenKind::Keyword(Keyword::Elseif)) {
            conditions.push(consume(stream)?);
        } else if matches!(next_token.kind, TokenKind::Keyword(Keyword::Else)) {
            conditions.push(consume(stream)?);
            break;
        } else {
            break;
        }
    }

    Ok(Expr::If {
        conditions: conditions,
    })
}
