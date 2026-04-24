use std::{iter::Peekable, vec::IntoIter};

use serenity::all::{Context, Message};

use crate::{
    commands::{CommandArgument, TransformerError, TransformerReturn},
    event_handler::MissingArgumentError,
    lexer::{InferType, Token},
    transformers::Transformers,
};

impl Transformers {
    pub fn reply_consume<'a>(
        ctx: &'a Context,
        msg: &'a Message,
        args: &'a mut Peekable<IntoIter<Token>>,
    ) -> TransformerReturn<'a> {
        Box::pin(async move {
            if args.peek().is_some() {
                return Transformers::consume(ctx, msg, args).await;
            } else if let Some(reply) = msg.referenced_message.clone() {
                if let Ok(Some(row)) =
                    sqlx::query("SELECT content FROM log_messages_context WHERE message_id = $1")
                        .bind(reply.id.get() as i64)
                        .fetch_optional(&*crate::SQL)
                        .await
                {
                    if let Some(content) =
                        sqlx::Row::try_get::<Option<String>, _>(&row, "content").unwrap_or(None)
                    {
                        return Ok(Token {
                            contents: Some(CommandArgument::String(content)),
                            raw: String::new(),
                            position: 0,
                            length: 0,
                            iteration: 0,
                            quoted: false,
                            inferred: Some(InferType::SystemMessage),
                        });
                    }
                }

                let (content, infer_type) = if let Some(embed) = reply.embeds.first() {
                    let desc = embed.clone().description.unwrap_or_default();

                    if embed.clone().kind.unwrap_or_default() == "auto_moderation_message" {
                        (format!("Automod: {desc}"), InferType::SystemMessage)
                    } else if desc.starts_with("**MESSAGE DELETED**")
                        || desc.starts_with("**MESSAGE EDITED**")
                    {
                        (
                            format!("Message: <Stored context lost>"),
                            InferType::SystemMessage,
                        )
                    } else {
                        (format!("Message: {}", reply.content), InferType::Message)
                    }
                } else {
                    (format!("Message: {}", reply.content), InferType::Message)
                };

                Ok(Token {
                    contents: Some(CommandArgument::String(content)),
                    raw: String::new(),
                    position: 0,
                    length: 0,
                    iteration: 0,
                    quoted: false,
                    inferred: Some(infer_type),
                })
            } else {
                Err(TransformerError::MissingArgumentError(
                    MissingArgumentError(String::from("String")),
                ))
            }
        })
    }
}
