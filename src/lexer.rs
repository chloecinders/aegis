use crate::commands::CommandArgument;

#[derive(Debug, Clone)]
pub enum InferType {
    Message,
    SystemMessage,
}

#[derive(Debug, Clone, Default)]
pub struct Token {
    pub contents: Option<CommandArgument>,
    pub raw: String,
    pub position: usize,
    pub length: usize,
    pub iteration: usize,
    pub quoted: bool,
    pub inferred: Option<InferType>,
}

pub fn lex(input: String) -> Vec<Token> {
    let mut iter = input.chars().enumerate().peekable();
    let mut tokens: Vec<Token> = vec![];
    let mut in_quotes = false;
    let mut quote_char = '\0';
    let mut was_quoted = false;
    let mut token_start = 0;
    let mut token_count = 0;
    let mut current_token = String::new();

    while let Some((i, ch)) = iter.next() {
        if ch == '\\' {
            if let Some((_, next_char)) = iter.next() {
                current_token.push(next_char);
            }
            continue;
        }

        if in_quotes {
            if ch == quote_char {
                in_quotes = false;
                continue;
            } else {
                current_token.push(ch);
                continue;
            }
        }

        if ch.is_whitespace() {
            if !current_token.is_empty() {
                tokens.push(Token {
                    contents: None,
                    raw: current_token.clone(),
                    position: token_start,
                    length: current_token.len(),
                    iteration: token_count,
                    quoted: was_quoted,
                    inferred: None,
                });
                token_count += 1;
                current_token.clear();
                was_quoted = false;
            }
            continue;
        }

        if ch == '"' || ch == '\'' {
            if current_token.is_empty() {
                in_quotes = true;
                quote_char = ch;
                token_start = i + 1;
                was_quoted = true;
                continue;
            } else {
                current_token.push(ch);
                continue;
            }
        }

        if current_token.is_empty() {
            token_start = i;
        }

        current_token.push(ch);
    }

    if !current_token.is_empty() {
        tokens.push(Token {
            contents: None,
            length: current_token.len(),
            raw: current_token,
            position: token_start,
            iteration: token_count,
            quoted: was_quoted,
            inferred: None,
        });
    }

    tokens
}
