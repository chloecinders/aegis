use std::{iter::Peekable, vec::IntoIter};

use serenity::all::{Context, Message};
use sqlx::Row;

use crate::{
    ENCRYPTION_KEYS,
    commands::{CommandArgument, TransformerError, TransformerReturn},
    event_handler::MissingArgumentError,
    lexer::{InferType, Token},
    transformers::Transformers,
    utils::encryption::decrypt,
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
                    sqlx::query("SELECT guild_id, content FROM log_messages_context WHERE message_id = $1")
                        .bind(reply.id.get() as i64)
                        .fetch_optional(&*crate::SQL)
                        .await
                {
                    let guild_id: i64 = row.try_get("guild_id").unwrap_or(0);
                    let content_bytes: Option<Vec<u8>> = row.try_get("content").unwrap_or(None);

                    let decrypted_content = if let Some(bytes) = content_bytes {
                        let lock = ENCRYPTION_KEYS.lock().await;
                        if let Some(key) = lock.get(&(guild_id as u64)) {
                            decrypt(key, &bytes).or_else(|| String::from_utf8(bytes.clone()).ok())
                        } else {
                            String::from_utf8(bytes).ok()
                        }
                    } else {
                        None
                    };

                    if let Some(content) = decrypted_content {
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
                        (desc, InferType::SystemMessage)
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
