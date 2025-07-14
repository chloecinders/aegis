use crate::{
    executor::primitives::{
        BoolPrimitive, FloatPrimitive, IntPrimitive, StringPrimitive, traits::PrimitiveValue,
    },
    lexer::{Keyword, Operator, Token, TokenKind, TokenStream},
    parser::parsers::{
        parse_if, parse_math, parse_sentence, parse_variable_assignment, parse_while, parse_word,
    },
};

use super::{AstNode, Program};

#[derive(Debug)]
pub enum ParseError {
    UnexpectedToken,
    TokenNotFound,
    MustBeExpr,
}

pub struct Parser;

impl Parser {
    pub fn parse(mut stream: TokenStream) -> Result<Program, ParseError> {
        let ast = Self::parse_stream(&mut stream)?;
        Ok(Program { ast })
    }

    pub fn parse_stream(mut stream: &mut TokenStream) -> Result<Vec<AstNode>, ParseError> {
        let mut ast: Vec<AstNode> = Vec::new();

        while let Some(token) = stream.peek() {
            if (*token).kind == TokenKind::Semicolon {
                stream.next();
                continue;
            }

            let result = Self::parse_single(&mut stream)?;
            ast.push(result);
        }

        Ok(ast)
    }

    pub fn parse_single(mut stream: &mut TokenStream) -> Result<AstNode, ParseError> {
        let mut chunk: Vec<Token> = vec![];
        let mut i = 0;

        while let Some(t) = stream.peek_next(i) {
            if let TokenKind::Semicolon = t.kind {
                break;
            }

            chunk.push(t.clone());
            i += 1;
        }

        let Some(token) = stream.next() else {
            return Err(ParseError::UnexpectedToken);
        };

        let result: Result<AstNode, ParseError> = match token.kind {
            TokenKind::Keyword(Keyword::If) => match parse_if(&mut stream) {
                Ok(e) => Ok(AstNode::Expr(e)),
                Err(e) => Err(e),
            },

            TokenKind::Keyword(Keyword::While) => match parse_while(&mut stream) {
                Ok(e) => Ok(AstNode::Expr(e)),
                Err(e) => Err(e),
            },

            TokenKind::Keyword(Keyword::Let) => match parse_variable_assignment(&mut stream) {
                Ok(e) => Ok(AstNode::Expr(e)),
                Err(e) => Err(e),
            },

            // Parse math/binary expressions
            _ if chunk.iter().len() > 2
                && chunk.iter().all(|t| match t.kind {
                    TokenKind::Int(_) => true,
                    TokenKind::Float(_) => true,
                    TokenKind::Word(_) => true,
                    TokenKind::OpenBracket => true,
                    TokenKind::ClosedBracket => true,
                    TokenKind::Operator(Operator::Equal) => false,
                    TokenKind::Operator(_) => true,
                    _ => false,
                }) =>
            {
                Ok(AstNode::Expr(parse_math(token, &mut stream)))
            }

            TokenKind::Word(s) => parse_word(&mut stream, s),
            TokenKind::Sentence(s) => parse_sentence(s),

            TokenKind::Int(i) => Ok(AstNode::Expr(IntPrimitive::from_value_to_expr(i))),
            TokenKind::Float(f) => Ok(AstNode::Expr(FloatPrimitive::from_value_to_expr(f))),
            TokenKind::String(s) => Ok(AstNode::Expr(StringPrimitive::from_value_to_expr(
                s.clone(),
            ))),
            TokenKind::Bool(b) => Ok(AstNode::Expr(BoolPrimitive::from_value_to_expr(b))),

            _ => Err(ParseError::UnexpectedToken),
        };

        result
    }
}
