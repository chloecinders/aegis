use std::sync::Arc;

use serenity::{
    all::{Color, Context, Message, MessageId, Permissions},
    async_trait,
};

use crate::{
    SQL,
    commands::{
        Command, CommandArgument, CommandCategory, CommandParameter, CommandPermissions,
        CommandSyntax, TransformerFnArc,
    },
    event_handler::{CommandError, Handler},
    lexer::Token,
    transformers::Transformers,
    utils::{consume_pgsql_error, consume_serenity_error, sticky_cache::StickyMessage},
};
use aegis_macros::command;

pub struct Sticky;

impl Sticky {
    pub fn new() -> Self {
        Self {}
    }
}

#[async_trait]
impl Command for Sticky {
    fn get_name(&self) -> &'static str {
        "sticky"
    }

    fn get_short(&self) -> &'static str {
        "Adds or deletes a sticky message"
    }

    fn get_full(&self) -> &'static str {
        "Adds or deletes a sticky message in the current channel. If no message is provided, the current sticky message is deleted. If title or color is provided, the message is sent as an embed."
    }

    fn get_syntax(&self) -> Vec<CommandSyntax> {
        vec![CommandSyntax::Consume("message")]
    }

    fn get_category(&self) -> CommandCategory {
        CommandCategory::Admin
    }

    fn get_params(&self) -> Vec<&'static CommandParameter<'static>> {
        vec![
            &CommandParameter {
                name: "title",
                short: "t",
                transformer: &Transformers::string_consume,
                desc: "The title for the sticky message embed (will be sent as caps)",
            },
            &CommandParameter {
                name: "color",
                short: "c",
                transformer: &Transformers::some_string,
                desc: "The hex color code for the sticky message embed (#04d9b2)",
            },
        ]
    }

    fn get_permissions(&self) -> CommandPermissions {
        CommandPermissions {
            required: vec![Permissions::ADMINISTRATOR],
            one_of: vec![],
            bot: CommandPermissions::baseline(),
            silence_typing: false,
        }
    }

    #[command]
    async fn run(
        &self,
        ctx: Context,
        msg: Message,
        handler: &Handler,
        #[transformers::consume] message: Option<String>,
        params: HashMap<&str, (bool, CommandArgument)>,
        trace: &mut TraceContext,
    ) -> Result<(), CommandError> {
        if msg.guild_id.is_none() {
            return Err(CommandError::new(
                "This command can only be used in a server.",
            ));
        }

        let channel_id = msg.channel_id;
        let message = message.filter(|m| !m.trim().is_empty());

        if message.is_none() {
            trace.point("removing_sticky");
            let removed = {
                let mut lock = handler.sticky_cache.lock().await;
                lock.remove_sticky(channel_id.get())
            };

            if let Some(old_sticky) = removed {
                if let Some(old_id) = old_sticky.last_message_id {
                    let _ = channel_id
                        .delete_message(&ctx, MessageId::new(old_id))
                        .await;
                }

                let _ = sqlx::query("DELETE FROM sticky_messages WHERE channel_id = $1")
                    .bind(channel_id.get() as i64)
                    .execute(&*SQL)
                    .await;

                let _ = msg.delete(&ctx).await;
            } else {
                return Err(CommandError::new(format!(
                    "No sticky message found in <#{channel_id}>."
                )));
            }

            return Ok(());
        }

        let content = message.unwrap();

        let title = params.get("title").and_then(|(active, arg)| {
            if *active {
                if let CommandArgument::String(s) = arg {
                    let trimmed = s.trim().to_string();
                    if trimmed.is_empty() {
                        None
                    } else {
                        Some(trimmed.to_uppercase())
                    }
                } else {
                    None
                }
            } else {
                None
            }
        });

        let color = params.get("color").and_then(|(active, arg)| {
            if *active {
                if let CommandArgument::String(s) = arg {
                    let s = s.trim().strip_prefix('#').unwrap_or(s.trim());
                    u32::from_str_radix(s, 16).ok().map(Color::new)
                } else {
                    None
                }
            } else {
                None
            }
        });

        trace.point("setting_sticky");

        let old_sticky = {
            let lock = handler.sticky_cache.lock().await;
            lock.get(channel_id.get())
        };

        if let Some(old) = old_sticky {
            if let Some(old_id) = old.last_message_id {
                let _ = channel_id
                    .delete_message(&ctx, MessageId::new(old_id))
                    .await;
            }
        }

        let _ = msg.delete(&ctx).await;

        let sticky_obj = StickyMessage {
            content: content.clone(),
            title: title.clone(),
            color,
            last_message_id: None,
        };

        let sent_msg = match channel_id
            .send_message(&ctx, sticky_obj.build_message())
            .await
        {
            Ok(m) => m,
            Err(err) => {
                consume_serenity_error("sending initial sticky message".into(), err);
                return Err(CommandError {
                    title: String::from("Failed to send sticky message to the channel."),
                    hint: Some(String::from("check the bot's permissions in this channel")),
                    arg: None,
                });
            }
        };

        let sent_id = sent_msg.id.get();

        if let Err(err) = sqlx::query(
            "INSERT INTO sticky_messages (channel_id, content, title, color, last_message_id) \
             VALUES ($1, $2, $3, $4, $5) \
             ON CONFLICT (channel_id) DO UPDATE SET content = $2, title = $3, color = $4, last_message_id = $5",
        )
        .bind(channel_id.get() as i64)
        .bind(&content)
        .bind(&title)
        .bind(color.map(|c| c.0 as i64))
        .bind(sent_id as i64)
        .execute(&*SQL)
        .await
        {
            consume_pgsql_error("INSERT STICKY MESSAGE".into(), err);
            return Err(CommandError::new("Database error while saving sticky message."));
        }

        {
            let mut lock = handler.sticky_cache.lock().await;
            lock.set_sticky(
                channel_id.get(),
                content,
                title,
                color,
                Some(sent_id),
            );
        }

        Ok(())
    }
}
