use std::collections::VecDeque;

use super::{Operator, token::Token, token_stream::TokenStream};

#[derive(Debug)]
pub enum LexerError {
    InvalidNumber(usize),
    UnterminatedString(usize),
}

pub struct Lexer;

impl Lexer {
    const KEYWORDS: [&'static str; 4] = ["if", "let", "elif", "else"];
    const CHAR_MAP: [(char, Token); 12] = [
        ('(', Token::OpenBracket),
        (')', Token::ClosedBracket),
        ('{', Token::OpenCurlyBracket),
        ('}', Token::ClosedCurlyBracket),
        ('.', Token::Period),
        (';', Token::Semicolon),
        ('+', Token::Operator(Operator::Plus)),
        ('-', Token::Operator(Operator::Minus)),
        ('*', Token::Operator(Operator::Multiply)),
        ('/', Token::Operator(Operator::Divide)),
        ('=', Token::Operator(Operator::Equal)),
        ('%', Token::Operator(Operator::Modulo)),
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
                tokens.push_back(
                    Self::CHAR_MAP
                        .iter()
                        .find(|c| c.0 == char)
                        .unwrap()
                        .1
                        .clone(),
                );
                chars.next();
                pos += 1;
            } else if char.is_ascii_digit() || char == '.' {
                tokens.push_back(Self::parse_number(&mut chars, &mut pos)?);
            } else if char == '"' || char == '\'' {
                tokens.push_back(Self::parse_string(&mut chars, &mut pos)?);
            } else {
                tokens.push_back(Self::parse_else(&mut chars, &mut pos, {
                    let last = tokens.iter().last();
                    last.is_none() || matches!(last, Some(Token::Semicolon))
                })?);
            }
        }

        Ok(TokenStream::new(tokens))
    }

    fn parse_number<I: Iterator<Item = char>>(
        chars: &mut std::iter::Peekable<I>,
        pos: &mut usize,
    ) -> Result<Token, LexerError> {
        let starting_pos = pos.clone();
        let mut number_str = String::new();

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
            let value: f64 = number_str
                .parse()
                .map_err(|_| LexerError::InvalidNumber(starting_pos))?;

            Ok(Token::Float(value))
        } else {
            let value: i64 = number_str
                .parse()
                .map_err(|_| LexerError::InvalidNumber(starting_pos))?;

            Ok(Token::Int(value))
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
            return Err(LexerError::UnterminatedString(starting_pos));
        }

        Ok(Token::String(full_string))
    }

    fn parse_else<I: Iterator<Item = char>>(
        chars: &mut std::iter::Peekable<I>,
        pos: &mut usize,
        is_alone: bool,
    ) -> Result<Token, LexerError> {
        let mut full_word = String::new();

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
            "true" => Some(Token::Bool(true)),
            "false" => Some(Token::Bool(false)),
            str if Self::KEYWORDS.contains(&str) => match str {
                "if" => Some(Token::Keyword(super::Keyword::If)),
                "let" => Some(Token::Keyword(super::Keyword::Let)),
                "elif" => Some(Token::Keyword(super::Keyword::Elseif)),
                "else" => Some(Token::Keyword(super::Keyword::Else)),
                _ => Some(Token::Word(full_word)),
            },
            _ => None,
        };

        if !is_alone {
            return Ok(Token::Word(String::from(copy.trim())));
        }

        if let Some(t) = res {
            return Ok(t);
        }

        while let Some(&char) = chars.peek() {
            if char == ';' {
                break;
            }

            sentence.push(char);
            chars.next();
            *pos += 1;
        }

        if sentence.trim().len() != copy.trim().len() {
            return Ok(Token::Sentence(sentence));
        }

        Ok(Token::Word(String::from(sentence.trim())))
    }

    fn is_char(char: char) -> bool {
        Self::CHAR_MAP.iter().find(|c| c.0 == char).is_some()
    }
}
