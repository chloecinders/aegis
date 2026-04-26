use std::sync::Arc;

use aegis_macros::command;
use serenity::{
    all::{Context, CreateAllowedMentions, CreateEmbed, CreateMessage, Message, Permissions},
    async_trait,
};
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
    utils::reference,
};

pub struct EditRef;

impl EditRef {
    pub fn new() -> Self {
        Self {}
    }

    async fn db_id_from_reply(msg: &Message) -> Option<String> {
        let reference = msg.message_reference.as_ref()?;
        let message_id = reference.message_id?.get();
        sqlx::query!(
            "SELECT db_id FROM log_messages_context WHERE message_id = $1",
            message_id as i64
        )
        .fetch_optional(&*SQL)
        .await
        .ok()
        .flatten()
        .and_then(|r| r.db_id)
    }
}

#[async_trait]
impl Command for EditRef {
    fn get_name(&self) -> &'static str {
        "edit_ref"
    }

    fn get_short(&self) -> &'static str {
        "Updates the reference for a moderation action"
    }

    fn get_full(&self) -> &'static str {
        "Replaces the saved reference (Discord message link or image URL) for a \
        moderation action. Provide the log ID and new URL, or reply to a log message \
        and provide just the new URL."
    }

    fn get_syntax(&self) -> Vec<CommandSyntax> {
        vec![
            CommandSyntax::String("id", false),
            CommandSyntax::Consume("new_ref"),
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
        let (action_id, new_ref_url) = if let Some(id) = EditRef::db_id_from_reply(&msg).await {
            let url = {
                let mut s = String::new();
                if let Some(a1) = arg1 {
                    s.push_str(&a1);
                }
                if let Some(a2) = arg2 {
                    if !s.is_empty() {
                        s.push(' ');
                    }
                    s.push_str(&a2);
                }
                s
            };
            (id, url)
        } else {
            let id = arg1.ok_or_else(|| CommandError {
                title: String::from("No action ID provided"),
                hint: Some(String::from("provide an ID or reply to a log message")),
                arg: None,
            })?;
            (id, arg2.unwrap_or_default())
        };

        if new_ref_url.trim().is_empty() {
            return Err(CommandError {
                title: String::from("No reference URL provided"),
                hint: Some(String::from(
                    "provide a Discord message link or an image URL",
                )),
                arg: None,
            });
        }

        trace.point("updating_ref");
        let ok = reference::update_ref(&ctx, &action_id, &new_ref_url).await;

        let description = if ok {
            format!("**`{action_id}` reference updated**\n-# Use `ref {action_id}` to view it.")
        } else {
            format!(
                "**Could not resolve reference**\n\
                -# Make sure the link is a Discord message URL or a direct image URL."
            )
        };

        let reply = CreateMessage::new()
            .add_embed(
                CreateEmbed::new()
                    .description(description)
                    .color(BRAND_BLUE),
            )
            .reference_message(&msg)
            .allowed_mentions(CreateAllowedMentions::new().replied_user(false));

        if let Err(err) = msg.channel_id.send_message(&ctx, reply).await {
            warn!("edit_ref: could not send message; err = {err:?}");
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
        }
    }
}
