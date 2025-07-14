use std::collections::VecDeque;

use crate::{lexer::TokenKind, parser::ParseError};

use super::token::Token;

#[derive(Debug)]
pub struct TokenStream {
    pub tokens: VecDeque<Token>,
}

impl TokenStream {
    pub fn new(stream: VecDeque<Token>) -> Self {
        Self { tokens: stream }
    }

    pub fn into_iter(self) -> impl Iterator<Item = Token> {
        self.tokens.into_iter()
    }

    pub fn peek(&self) -> Option<&Token> {
        self.tokens.get(0)
    }

    pub fn peek_next(&self, n: usize) -> Option<&Token> {
        self.tokens.get(n)
    }

    pub fn next(&mut self) -> Option<Token> {
        if self.len() == 0 {
            return None;
        }

        let current = self.tokens.get(0).cloned();
        self.tokens.remove(0);
        current
    }

    pub fn expect(&mut self, expected: TokenKind) -> Result<(), ParseError> {
        match self.next() {
            Some(Token { kind: token, .. }) if token == expected => Ok(()),
            _ => Err(ParseError::TokenNotFound),
        }
    }

    pub fn len(&self) -> usize {
        self.tokens.len()
    }

    pub fn push_front(&mut self, token: Token) {
        self.tokens.push_front(token);
    }
}
