use crate::{
    executor::primitives::{
        BoolPrimitive, FloatPrimitive, IntPrimitive, StringPrimitive, traits::PrimitiveValue,
    },
    lexer::{Keyword, Token, TokenStream},
    parser::parsers::{parse_if_expr, parse_math_expr, parse_sentence, parse_variable_assignment},
};

use super::{AstNode, Expr, Program};

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
        let mut chunk: Vec<Token> = vec![];
        let mut i = 0;

        while let Some(t) = stream.peek_next(i) {
            if let Token::Semicolon = t {
                break;
            }

            chunk.push(t.clone());
            i += 1;
        }

        let token = stream.next();

        let result: Result<AstNode, ParseError> = match token {
            // Parse any word/sentence (vars, functions, cmds)
            Some(Token::Word(s)) => parse_sentence(&mut stream, s),
            Some(Token::Sentence(s)) => parse_sentence(&mut stream, s),

            // Parse if statements
            Some(Token::Keyword(Keyword::If)) => match parse_if_expr(&mut stream) {
                Ok(e) => Ok(AstNode::Expr(e)),
                Err(e) => Err(e),
            },

            // Parse variable assignments
            Some(Token::Keyword(Keyword::Let)) => match parse_variable_assignment(&mut stream) {
                Ok(e) => Ok(AstNode::Expr(e)),
                Err(e) => Err(e),
            },

            // Parse math/binary expressions
            Some(token)
                if chunk.iter().len() > 2
                    && chunk.iter().all(|t| match t {
                        Token::Int(_) => true,
                        Token::Float(_) => true,
                        Token::Word(_) => true,
                        Token::OpenBracket => true,
                        Token::ClosedBracket => true,
                        Token::Operator(_) => true,
                        _ => false,
                    }) =>
            {
                Ok(AstNode::Expr(parse_math_expr(token, &mut stream)))
            }

            // Parse Primitives
            Some(Token::Int(i)) => Ok(AstNode::Expr(IntPrimitive::from_value_to_expr(i))),
            Some(Token::Float(f)) => Ok(AstNode::Expr(FloatPrimitive::from_value_to_expr(f))),
            Some(Token::String(s)) => Ok(AstNode::Expr(StringPrimitive::from_value_to_expr(
                s.clone(),
            ))),
            Some(Token::Bool(b)) => Ok(AstNode::Expr(BoolPrimitive::from_value_to_expr(b))),

            _ => Err(ParseError::UnexpectedToken),
        };

        let iter = stream.into_iter().peekable();
        let new_stream = TokenStream::new(iter.collect());

        (new_stream, result)
    }
}
