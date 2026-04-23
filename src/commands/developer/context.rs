use std::sync::Arc;

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
    utils::is_developer,
};
use ouroboros_macros::command;
use serenity::{
    all::{Context, CreateAllowedMentions, CreateEmbed, CreateMessage, Message},
    async_trait,
};

pub struct ContextCmd;

impl ContextCmd {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl Command for ContextCmd {
    fn get_name(&self) -> &'static str {
        "context"
    }

    fn get_short(&self) -> &'static str {
        "Shows log message context"
    }

    fn get_full(&self) -> &'static str {
        "Shows the context of a moderation log message from the replied message."
    }

    fn get_syntax(&self) -> Vec<CommandSyntax> {
        vec![CommandSyntax::String("message_id", true)]
    }

    fn get_category(&self) -> CommandCategory {
        CommandCategory::Developer
    }

    #[command]
    async fn run(
        &self,
        ctx: SerenityContext,
        msg: Message,
        #[transformers::string] message_id_arg: Option<String>,
        trace: &mut TraceContext,
    ) -> Result<(), CommandError> {
        if !is_developer(&msg.author) {
            return Ok(());
        }

        let message_id = if let Some(id) = message_id_arg {
            id.parse::<u64>().map_err(|_| CommandError {
                title: String::from("Invalid Message ID"),
                hint: Some(String::from("it must be a numeric ID")),
                arg: None,
            })?
        } else if let Some(referenced_message) = msg.message_reference.clone() {
            referenced_message
                .message_id
                .ok_or_else(|| CommandError {
                    title: String::from("Could not get ID from reply"),
                    hint: None,
                    arg: None,
                })?
                .get()
        } else {
            return Err(CommandError {
                title: String::from("You must provide a Message ID or reply to a log message."),
                hint: None,
                arg: None,
            });
        };

        trace.point("fetching_context_from_db");

        let context_record = sqlx::query!(
            "SELECT guild_id, target_id, moderator_id, db_id, content FROM log_messages_context WHERE message_id = $1",
            message_id as i64
        )
        .fetch_optional(&*SQL)
        .await
        .map_err(|err| {
            tracing::error!("DB error fetching context: {err:?}");
            CommandError {
                title: String::from("Database error while fetching context"),
                hint: None,
                arg: None,
            }
        })?;

        if let Some(row) = context_record {
            let mut description = format!(
                "**MESSAGE CONTEXT**\n-# Target: <@{0}> | Mod: <@{1}> | Guild: `{2}`\n",
                row.target_id, row.moderator_id, row.guild_id
            );

            if let Some(db_id) = &row.db_id {
                description.push_str(&format!("`{}`\n", db_id));
            }

            let mut reply = CreateMessage::new()
                .reference_message(&msg)
                .allowed_mentions(CreateAllowedMentions::new().replied_user(false));

            if let Some(content) = &row.content {
                if content.len() > 1500 {
                    reply = reply.add_file(serenity::all::CreateAttachment::bytes(
                        content.as_bytes(),
                        "content.txt",
                    ));
                } else {
                    description.push_str(&format!(
                        "```\n{}\n```\n\n",
                        content.replace("```", "\\`\\`\\`")
                    ));
                }
            }

            reply = reply.add_embed(
                CreateEmbed::new()
                    .description(description)
                    .color(BRAND_BLUE),
            );

            let _ = msg.channel_id.send_message(&ctx, reply).await;
        } else {
            return Err(CommandError {
                title: String::from("No log context found for that message."),
                hint: None,
                arg: None,
            });
        }

        Ok(())
    }

    fn get_params(&self) -> Vec<&'static CommandParameter<'static>> {
        vec![]
    }

    fn get_permissions(&self) -> CommandPermissions {
        CommandPermissions {
            required: vec![],
            one_of: vec![],
            bot: CommandPermissions::baseline(),
        }
    }
}
