use std::iter::{Peekable, Successors};

use crate::{
    executor::primitives::{FloatPrimitive, IntPrimitive, traits::PrimitiveValue},
    lexer::{self, Keyword, Token, TokenStream},
    parser::Operator,
};

use super::{AstNode, Cmd, Expr, Program};

#[derive(Debug)]
pub enum ParseError {
    UnexpectedToken,
    TokenNotFound,
    MustBeExpr,
}

pub struct Parser;

impl Parser {
    pub fn parse(mut stream: TokenStream) -> Result<Program, ParseError> {
        let mut ast: Vec<AstNode> = Vec::new();

        while let Some(token) = stream.peek() {
            if *token == Token::Semicolon {
                stream.next();
                continue;
            }

            let result = Self::parse_stmt_borrow(stream);
            stream = result.0;
            ast.push(result.1?);
        }

        Ok(Program { ast })
    }

    pub fn parse_stmt_borrow(
        mut stream: TokenStream,
    ) -> (TokenStream, Result<AstNode, ParseError>) {
        let result: Result<AstNode, ParseError> = match stream.next() {
            Some(Token::Word(_)) => Self::parse_word(&stream),
            Some(Token::Keyword(Keyword::If)) => match Self::parse_if_expr(&mut stream) {
                Ok(e) => Ok(AstNode::Expr(e)),
                Err(e) => Err(e),
            },
            Some(token)
                if matches!(stream.peek(), Some(Token::Operator(_)))
                    || matches!(stream.peek_next(1), Some(Token::Operator(_))) =>
            {
                match Self::parse_op(token, &mut stream) {
                    Ok(e) => Ok(AstNode::Expr(e)),
                    Err(e) => Err(e),
                }
            }
            Some(Token::Int(i)) => Ok(AstNode::Expr(IntPrimitive::from_value_to_expr(i))),
            Some(Token::Float(f)) => Ok(AstNode::Expr(FloatPrimitive::from_value_to_expr(f))),
            Some(Token::String(s)) => Ok(AstNode::Expr(Expr::String(s.clone()))),
            Some(Token::Bool(b)) => Ok(AstNode::Expr(Expr::Bool(b))),
            _ => Err(ParseError::UnexpectedToken),
        };

        let iter = stream.into_iter().peekable();
        let new_stream = TokenStream::new(iter.collect());

        if new_stream.len() != 0 && new_stream.peek().map_or(true, |t| *t != Token::Semicolon) {
            (new_stream, Err(ParseError::UnexpectedToken))
        } else {
            (new_stream, result)
        }
    }

    fn parse_if_expr(stream: &mut TokenStream) -> Result<Expr, ParseError> {
        let mut cond_vec: Vec<Token> = Vec::new();

        while let Some(token) = stream.peek() {
            if *token == Token::OpenCurlyBracket {
                break;
            }

            cond_vec.push(stream.next().unwrap());
        }

        let cond_stream = TokenStream::new(cond_vec);

        if let Some(Token::Word(_)) = cond_stream.peek() {
            return Err(ParseError::MustBeExpr);
        }

        stream.expect(Token::OpenCurlyBracket)?;

        let mut body_vec: Vec<Token> = Vec::new();

        while let Some(token) = stream.peek() {
            if *token == Token::ClosedCurlyBracket {
                break;
            }

            body_vec.push(stream.next().unwrap());
        }

        stream.expect(Token::ClosedCurlyBracket)?;

        let cond_expr = {
            if let Ok(AstNode::Expr(expr)) = Self::parse_stmt_borrow(cond_stream).1 {
                Ok(expr)
            } else {
                Err(ParseError::UnexpectedToken)
            }
        }?;

        let program = Self::parse(TokenStream::new(body_vec))?;

        Ok(Expr::If {
            condition: Box::new(cond_expr),
            body: program.ast.into_iter().map(|expr| Box::new(expr)).collect(),
        })
    }

    fn parse_op(leading: Token, stream: &mut TokenStream) -> Result<Expr, ParseError> {
        let left = Self::parse_stmt_borrow(TokenStream::new(vec![leading])).1?;

        if stream.peek().is_none() {
            return Err(ParseError::UnexpectedToken);
        }

        let op = Self::parse_operator_token(stream.next().unwrap())?;

        if stream.peek().is_none() {
            return Err(ParseError::UnexpectedToken);
        }

        let right = Self::parse_stmt_borrow(TokenStream::new(vec![stream.next().unwrap()])).1?;

        if let AstNode::Expr(left) = left
            && let AstNode::Expr(right) = right
        {
            Ok(Expr::Operation {
                left: Box::new(left),
                op,
                right: Box::new(right),
            })
        } else {
            Err(ParseError::UnexpectedToken)
        }
    }

    fn parse_word(stream: &TokenStream) -> Result<AstNode, ParseError> {
        todo!();
    }

    fn parse_operator_token(token: Token) -> Result<Operator, ParseError> {
        match token {
            Token::Operator(lexer::Operator::Plus) => Ok(Operator::Add),
            Token::Operator(lexer::Operator::Minus) => Ok(Operator::Subtract),
            Token::Operator(lexer::Operator::Divide) => Ok(Operator::Divide),
            Token::Operator(lexer::Operator::Multiply) => Ok(Operator::Multiply),
            _ => Err(ParseError::UnexpectedToken),
        }
    }
}
