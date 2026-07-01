use std::{sync::Arc, time::Duration};

use aegis_macros::command;
use serenity::{
    all::{Context, CreateAllowedMentions, CreateEmbed, CreateMessage, Message, Permissions},
    async_trait,
};
use sqlx::query;
use tracing::warn;

use crate::{
    SQL,
    commands::{
        Command, CommandArgument, CommandCategory, CommandParameter, CommandPermissions,
        CommandSyntax, TransformerFnArc,
    },
    constants::BRAND_BLUE,
    event_handler::{CommandError, Handler},
    lexer::Token,
    transformers::Transformers,
    utils::update_guild_log,
};

pub struct Reason;

impl Reason {
    pub fn new() -> Self {
        Self {}
    }
}

#[async_trait]
impl Command for Reason {
    fn get_name(&self) -> &'static str {
        "reason"
    }

    fn get_short(&self) -> &'static str {
        "Modifies the reason of a moderation action"
    }

    fn get_full(&self) -> &'static str {
        "Modifies the reason of a moderation action. Run the log command for the id."
    }

    fn get_syntax(&self) -> Vec<CommandSyntax> {
        vec![
            CommandSyntax::String("id", false),
            CommandSyntax::Consume("reason"),
        ]
    }

    fn get_category(&self) -> CommandCategory {
        CommandCategory::Moderation
    }

    fn get_params(&self) -> Vec<&'static CommandParameter<'static>> {
        vec![]
    }

    #[command]
    async fn run(
        &self,
        ctx: Context,
        msg: Message,
        #[transformers::string] arg1: Option<String>,
        #[transformers::consume] arg2: Option<String>,
        trace: &mut TraceContext,
    ) -> Result<(), CommandError> {
        let mut db_id = None;
        if let Some(reference) = &msg.message_reference {
            let message_id = reference.message_id.unwrap().get();
            if let Ok(Some(record)) = sqlx::query!(
                "SELECT db_id FROM log_messages_context WHERE message_id = $1",
                message_id as i64
            )
            .fetch_optional(&*SQL)
            .await
            {
                db_id = record.db_id;
            }
        }

        let (id, mut reason) = if let Some(id) = db_id.clone() {
            let mut r = String::new();
            if let Some(a1) = arg1 {
                r.push_str(&a1);
            }
            if let Some(a2) = arg2 {
                if !r.is_empty() {
                    r.push(' ');
                }
                r.push_str(&a2);
            }
            (id, r)
        } else {
            let id = arg1.ok_or_else(|| {
                CommandError::arg_not_found(
                    "id",
                    Some("please provide an ID or reply to a log message"),
                )
            })?;
            (id, arg2.unwrap_or_default())
        };

        if reason.is_empty() || reason.chars().all(char::is_whitespace) {
            reason = String::from("No reason provided");
        }

        if reason.len() > 500 {
            reason.truncate(500);
            reason.push_str("...");
        }

        trace.point("updating_database");

        let res = query!(
            r#"
                UPDATE actions SET reason = $1, updated_at = NOW() WHERE guild_id = $2 AND id = $3 RETURNING id, reason, note;
            "#,
            reason,
            msg.guild_id.map(|g| g.get()).unwrap_or(0) as i64,
            id
        ).fetch_optional(&*SQL).await;

        let data = match res {
            Ok(d) => d,
            Err(err) => {
                warn!("Couldn't fetch log data; err = {err:?}");
                return Err(CommandError {
                    title: String::from("Unable to query the database"),
                    hint: Some(String::from("try again later")),
                    arg: None,
                });
            }
        };

        let Some(data) = data else {
            return Err(CommandError {
                title: String::from("Log not found"),
                hint: Some(String::from("check if you have copied the ID correctly!")),
                arg: None,
            });
        };

        let note_suffix = data
            .note
            .as_deref()
            .map(|n| format!("\n-# \u{1F4DD} Note: {n}"))
            .unwrap_or_default();

        let reply = CreateMessage::new()
            .add_embed(
                CreateEmbed::new()
                    .description(format!(
                        "**`{id}` UPDATED**```\n{}\n```{note_suffix}",
                        data.reason
                    ))
                    .color(BRAND_BLUE),
            )
            .reference_message(&msg)
            .allowed_mentions(CreateAllowedMentions::new().replied_user(false));

        let res_msg = match msg.channel_id.send_message(&ctx, reply).await {
            Ok(m) => Some(m),
            Err(err) => {
                warn!("Could not send message; err = {err:?}");
                None
            }
        };

        update_guild_log(&ctx, msg.guild_id.unwrap(), &id).await;

        if db_id.is_some() {
            if let Some(res_msg) = res_msg {
                tokio::spawn(async move {
                    tokio::time::sleep(Duration::from_secs(5)).await;
                    let _ = tokio::join!(msg.delete(&ctx), res_msg.delete(&ctx));
                });
            }
        }

        Ok(())
    }

    fn get_permissions(&self) -> CommandPermissions {
        CommandPermissions {
            required: vec![],
            one_of: vec![
                Permissions::MANAGE_NICKNAMES,
                Permissions::KICK_MEMBERS,
                Permissions::MODERATE_MEMBERS,
                Permissions::BAN_MEMBERS,
            ],
            bot: CommandPermissions::baseline(),
            silence_typing: false,
        }
    }
}
