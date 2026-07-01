use std::{iter::Peekable, vec::IntoIter};

use chrono::Duration;
use serenity::all::{Context, Message};

use crate::{
    commands::{CommandArgument, TransformerError, TransformerReturn},
    event_handler::{CommandError, MissingArgumentError},
    lexer::Token,
    transformers::Transformers,
};

impl Transformers {
    pub fn duration<'a>(
        _ctx: &'a Context,
        _msg: &'a Message,
        args: &'a mut Peekable<IntoIter<Token>>,
    ) -> TransformerReturn<'a> {
        Box::pin(async move {
            let Some(mut input) = args.next() else {
                return Err(TransformerError::MissingArgumentError(
                    MissingArgumentError(String::from("Duration")),
                ));
            };

            let s = input.raw.clone();

            if s == "0" {
                input.contents = Some(CommandArgument::Duration(Duration::default()));
                return Ok(input);
            } else if s.chars().all(|c| c.is_ascii_digit()) {
                if let Ok(numbers) = s.parse::<i64>() {
                    input.contents = Some(CommandArgument::Duration(Duration::seconds(numbers)));
                    return Ok(input);
                }
            } else if s.chars().count() < 2 {
                return Err(TransformerError::CommandError(CommandError {
                    arg: Some(input),
                    title: String::from("Could not turn input to a <Duration>"),
                    hint: Some(String::from(
                        "provide a valid number and a unit (s, m, h, d, w, M, y), i.e. 1h (1 hour) or 25d (25 days), or duration in seconds",
                    )),
                }));
            }

            let (digits, last) = s.split_at(s.len() - 1);

            if !"smhdwMy".contains(last) {
                return Err(TransformerError::CommandError(CommandError {
                    arg: Some(input),
                    title: String::from("Could not turn input to a <Duration>"),
                    hint: Some(String::from(
                        "provide a valid number and a unit (s, m, h, d, w, M, y), i.e. 1h (1 hour) or 25d (25 days)",
                    )),
                }));
            }

            let Ok(numbers) = digits.parse::<u32>() else {
                return Err(TransformerError::CommandError(CommandError {
                    arg: Some(input),
                    title: String::from("Could not turn input to a <Duration>"),
                    hint: Some(String::from(
                        "provide a valid number and a unit (s, m, h, d, w, M, y), i.e. 1h (1 hour) or 25d (25 days)",
                    )),
                }));
            };

            let numbers = numbers as i64;

            match last {
                "s" => input.contents = Some(CommandArgument::Duration(Duration::seconds(numbers))),
                "m" => input.contents = Some(CommandArgument::Duration(Duration::minutes(numbers))),
                "h" => input.contents = Some(CommandArgument::Duration(Duration::hours(numbers))),
                "d" => input.contents = Some(CommandArgument::Duration(Duration::days(numbers))),
                "w" => input.contents = Some(CommandArgument::Duration(Duration::weeks(numbers))),
                "M" => {
                    input.contents = Some(CommandArgument::Duration(Duration::days(numbers * 30)))
                }
                "y" => {
                    input.contents = Some(CommandArgument::Duration(Duration::days(numbers * 365)))
                }
                _ => {
                    return Err(TransformerError::CommandError(CommandError {
                        arg: Some(input),
                        title: String::from("Could not turn input to a <Duration>"),
                        hint: Some(String::from(
                            "provide a valid number and a unit (s, m, h, d, w, M, y), i.e. 1h (1 hour) or 25d (25 days)",
                        )),
                    }));
                }
            };

            Ok(input)
        })
    }
}
