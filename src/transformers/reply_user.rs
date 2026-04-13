use std::{iter::Peekable, vec::IntoIter};

use serenity::all::{Context, Message, MessageType};
use sqlx::Row;

use crate::{
    commands::{CommandArgument, TransformerError, TransformerReturn},
    event_handler::CommandError,
    lexer::{InferType, Token},
    transformers::Transformers,
};

impl Transformers {
    pub fn reply_user<'a>(
        ctx: &'a Context,
        msg: &'a Message,
        args: &'a mut Peekable<IntoIter<Token>>,
    ) -> TransformerReturn<'a> {
        Box::pin(async move {
            if msg.guild_id.is_none() {
                return Err(TransformerError::CommandError(CommandError {
                    title: String::from("Server only command"),
                    hint: Some(String::from("stop trying to run this in dms!")),
                    arg: None,
                }));
            }

            if let Some(reply) = msg.referenced_message.clone() {
                let mut target_user = reply.author.clone();

                if let Ok(Some(row)) = sqlx::query(
                    "SELECT target_id FROM log_messages_context WHERE message_id = $1"
                )
                .bind(reply.id.get() as i64)
                .fetch_optional(&*crate::SQL).await {
                    let target_id_i64: i64 = row.try_get("target_id").unwrap_or(0);
                    if target_id_i64 > 0 {
                        if let Ok(user) = ctx.http.get_user((target_id_i64 as u64).into()).await {
                            target_user = user;
                        }
                    }
                }

                if target_user.id == ctx.cache.current_user().id {
                    return Err(TransformerError::CommandError(CommandError {
                        title: String::from("Cannot infer user from log"),
                        hint: Some(String::from("This log is missing valid database context and is targeting the bot instead. Please provide the user explicitly.")),
                        arg: None,
                    }));
                }

                let infer_type = if matches!(reply.kind, MessageType::AutoModAction) {
                    InferType::SystemMessage
                } else {
                    InferType::Message
                };

                Ok(Token {
                    contents: Some(CommandArgument::User(target_user)),
                    raw: String::new(),
                    position: 0,
                    length: 0,
                    iteration: 0,
                    quoted: false,
                    inferred: Some(infer_type),
                })
            } else {
                return Transformers::user(ctx, msg, args).await;
            }
        })
    }
}
