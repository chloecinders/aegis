use crate::parser::ParseError;

use super::token::Token;

#[derive(Debug)]
pub struct TokenStream {
    pub tokens: Vec<Token>,
}

impl TokenStream {
    pub fn new(stream: Vec<Token>) -> Self {
        Self { tokens: stream }
    }

    pub fn iter(&self) -> impl Iterator<Item = &Token> {
        self.tokens.iter()
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

    pub fn expect(&mut self, expected: Token) -> Result<(), ParseError> {
        match self.next() {
            Some(token) if token == expected => Ok(()),
            _ => Err(ParseError::TokenNotFound),
        }
    }

    pub fn len(&self) -> usize {
        self.tokens.len()
    }
}
