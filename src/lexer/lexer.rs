use std::collections::VecDeque;

use crate::lexer::Token;

use super::{Operator, token::TokenKind, token_stream::TokenStream};

#[derive(Debug)]
pub enum LexerError {
    InvalidNumber,
    UnterminatedString,
}

pub struct Lexer;

impl Lexer {
    const CHAR_MAP: [(char, TokenKind); 12] = [
        ('(', TokenKind::OpenBracket),
        (')', TokenKind::ClosedBracket),
        ('{', TokenKind::OpenCurlyBracket),
        ('}', TokenKind::ClosedCurlyBracket),
        ('.', TokenKind::Period),
        (';', TokenKind::Semicolon),
        ('+', TokenKind::Operator(Operator::Plus)),
        ('-', TokenKind::Operator(Operator::Minus)),
        ('*', TokenKind::Operator(Operator::Multiply)),
        ('/', TokenKind::Operator(Operator::Divide)),
        ('=', TokenKind::Operator(Operator::Equal)),
        ('%', TokenKind::Operator(Operator::Modulo)),
    ];

    pub fn parse<'a>(input: &'a str) -> Result<TokenStream, LexerError> {
        let mut tokens: VecDeque<Token> = VecDeque::new();
        let mut chars = input.chars().peekable();
        let mut pos: usize = 0;

        while let Some(&char) = chars.peek() {
            if char.is_whitespace() {
                chars.next();
                pos += 1;
            } else if Self::is_char(char) {
                tokens.push_back(Token {
                    kind: Self::CHAR_MAP
                        .iter()
                        .find(|c| c.0 == char)
                        .unwrap()
                        .1
                        .clone(),
                    pos: pos.clone(),
                });
                chars.next();
                pos += 1;
            } else if char.is_ascii_digit() || char == '.' {
                tokens.push_back(Self::parse_number(&mut chars, &mut pos)?);
            } else if char == '"' || char == '\'' {
                tokens.push_back(Self::parse_string(&mut chars, &mut pos)?);
            } else {
                tokens.push_back(Self::parse_else(&mut chars, &mut pos)?);
            }
        }

        Ok(TokenStream::new(tokens))
    }

    pub fn parse_strings<'a>(input: &'a str) -> Result<TokenStream, LexerError> {
        let mut tokens: VecDeque<Token> = VecDeque::new();
        let mut chars = input.chars().peekable();
        let mut pos: usize = 0;

        while let Some(&char) = chars.peek() {
            if char.is_whitespace() {
                chars.next();
                pos += 1;
            } else if char == '"' || char == '\'' {
                tokens.push_back(Self::parse_string(&mut chars, &mut pos)?);
            } else {
                tokens.push_back(Self::parse_word(&mut chars, &mut pos));
            }
        }

        Ok(TokenStream::new(tokens))
    }

    fn parse_number<I: Iterator<Item = char>>(
        chars: &mut std::iter::Peekable<I>,
        pos: &mut usize,
    ) -> Result<Token, LexerError> {
        let mut number_str = String::new();
        let current_pos = pos.clone();

        while let Some(&char) = chars.peek() {
            if char.is_ascii_alphanumeric() || char == '.' || char == '_' {
                number_str.push(char);
                *pos += 1;
                chars.next();
            } else {
                break;
            }
        }

        number_str.remove_matches("_");

        if number_str.contains('.') {
            let value: f64 = number_str.parse().map_err(|_| LexerError::InvalidNumber)?;

            Ok(Token {
                kind: TokenKind::Float(value),
                pos: current_pos,
            })
        } else {
            let value: i64 = number_str.parse().map_err(|_| LexerError::InvalidNumber)?;

            Ok(Token {
                kind: TokenKind::Int(value),
                pos: current_pos,
            })
        }
    }

    fn parse_string<I: Iterator<Item = char>>(
        chars: &mut std::iter::Peekable<I>,
        pos: &mut usize,
    ) -> Result<Token, LexerError> {
        let mut terminated = false;
        let mut full_string = String::new();
        let starting_pos = pos.clone();

        chars.next();
        while let Some(char) = chars.next() {
            if char == '"' || char == '\'' {
                terminated = true;
                *pos += 1;
                break;
            }

            full_string.push(char);
            *pos += 1;
        }

        if !terminated {
            return Err(LexerError::UnterminatedString);
        }

        Ok(Token {
            kind: TokenKind::String(full_string),
            pos: starting_pos,
        })
    }

    fn parse_else<I: Iterator<Item = char>>(
        chars: &mut std::iter::Peekable<I>,
        pos: &mut usize,
    ) -> Result<Token, LexerError> {
        let mut full_word = String::new();
        let current_pos = pos.clone();

        while let Some(&char) = chars.peek() {
            if char.is_whitespace() || Self::is_char(char) {
                break;
            }

            full_word.push(char);
            chars.next();
            *pos += 1;
        }

        let mut sentence = full_word.clone();
        let copy = full_word.clone();

        let res = match full_word.as_str().to_lowercase().as_str() {
            "true" => Some(TokenKind::Bool(true)),
            "false" => Some(TokenKind::Bool(false)),
            "if" => Some(TokenKind::Keyword(super::Keyword::If)),
            "let" => Some(TokenKind::Keyword(super::Keyword::Let)),
            "elif" => Some(TokenKind::Keyword(super::Keyword::Elseif)),
            "else" => Some(TokenKind::Keyword(super::Keyword::Else)),
            "fn" => Some(TokenKind::Keyword(super::Keyword::Fn)),
            "while" => Some(TokenKind::Keyword(super::Keyword::While)),
            _ => None,
        };

        if let Some(t) = res {
            return Ok(Token {
                kind: t,
                pos: current_pos,
            });
        }

        let mut checked = false;

        'outer: while let Some(&char) = chars.peek() {
            if char == ';' {
                break;
            }

            while !checked
                && char.is_whitespace()
                && let Some(inner_char) = chars.peek()
            {
                if char == ';' {
                    break 'outer;
                }

                if !inner_char.is_alphanumeric() && !inner_char.is_whitespace() {
                    return Ok(Token {
                        kind: TokenKind::Word(String::from(copy.trim())),
                        pos: current_pos,
                    });
                }

                if inner_char.is_alphabetic() {
                    checked = true;
                    continue 'outer;
                }

                sentence.push(*inner_char);
                chars.next();
            }

            sentence.push(char);
            chars.next();
            *pos += 1;
        }

        if sentence.trim().len() != copy.trim().len() {
            return Ok(Token {
                kind: TokenKind::Sentence(sentence),
                pos: current_pos,
            });
        }

        Ok(Token {
            kind: TokenKind::Word(String::from(sentence.trim())),
            pos: current_pos,
        })
    }

    fn parse_word<I: Iterator<Item = char>>(
        chars: &mut std::iter::Peekable<I>,
        pos: &mut usize,
    ) -> Token {
        let mut full_word = String::new();
        let current_pos = pos.clone();

        while let Some(&char) = chars.peek() {
            if char.is_whitespace() {
                break;
            }

            full_word.push(char);
            chars.next();
            *pos += 1;
        }

        Token {
            kind: TokenKind::Word(full_word),
            pos: current_pos,
        }
    }

    fn is_char(char: char) -> bool {
        Self::CHAR_MAP.iter().find(|c| c.0 == char).is_some()
    }
}
