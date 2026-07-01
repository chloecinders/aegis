use std::{iter::Peekable, vec::IntoIter};

use serenity::all::{Context, Message};

use crate::{
    commands::{CommandArgument, TransformerError, TransformerReturn},
    event_handler::{CommandError, MissingArgumentError},
    lexer::Token,
    transformers::Transformers,
};

impl Transformers {
    pub fn string_consume<'a>(
        _ctx: &'a Context,
        _msg: &'a Message,
        args: &'a mut Peekable<IntoIter<Token>>,
    ) -> TransformerReturn<'a> {
        Box::pin(async move {
            let mut consumed_tokens = Vec::new();

            while let Some(token) = args.peek() {
                if !token.quoted
                    && (token.raw.starts_with('-') || token.raw.starts_with('+'))
                    && token.raw.len() >= 2
                    && token.raw.chars().nth(1).map_or(false, |c| c.is_alphabetic())
                {
                    break;
                }
                consumed_tokens.push(args.next().unwrap());
            }

            if consumed_tokens.is_empty() {
                return Err(TransformerError::MissingArgumentError(
                    MissingArgumentError(String::from("String")),
                ));
            }

            let first_token = consumed_tokens.first().unwrap().clone();
            let joined = consumed_tokens
                .iter()
                .map(|t| {
                    if t.quoted && consumed_tokens.len() > 1 {
                        format!("\"{}\"", t.raw)
                    } else {
                        t.raw.clone()
                    }
                })
                .collect::<Vec<_>>()
                .join(" ");

            if joined.chars().all(char::is_whitespace) || joined.is_empty() {
                return Err(TransformerError::CommandError(CommandError {
                    arg: Some(first_token),
                    title: String::from("String must not be empty and not be whitespace"),
                    hint: None,
                }));
            }

            let mut result_token = first_token;
            result_token.raw = joined.clone();
            result_token.length = joined.len();
            result_token.contents = Some(CommandArgument::String(joined));
            Ok(result_token)
        })
    }
}
