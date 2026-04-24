use std::sync::Arc;

use serenity::{
    all::{Context, CreateAllowedMentions, CreateEmbed, CreateMessage, Message},
    async_trait,
};

use crate::{
    ENCRYPTION_KEYS, SQL,
    commands::{
        Command, CommandArgument, CommandCategory, CommandParameter, CommandPermissions,
        CommandSyntax, TransformerFnArc,
    },
    constants::BRAND_BLUE,
    event_handler::{CommandError, Handler},
    lexer::Token,
    transformers::Transformers,
    utils::encryption::decrypt,
};
use ouroboros_macros::command;

pub struct Edits;

impl Edits {
    pub fn new() -> Self {
        Self {}
    }
}

#[async_trait]
impl Command for Edits {
    fn get_name(&self) -> &'static str {
        "edits"
    }

    fn get_short(&self) -> &'static str {
        "Check message edit history"
    }

    fn get_full(&self) -> &'static str {
        "Shows all recorded edits for a specific message."
    }

    fn get_syntax(&self) -> Vec<CommandSyntax> {
        vec![CommandSyntax::String("message_id", true)]
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
        #[transformers::string] message_id_arg: Option<String>,
        trace: &mut TraceContext,
    ) -> Result<(), CommandError> {
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
                title: String::from("You must provide a Message ID or reply to a message."),
                hint: None,
                arg: None,
            });
        };

        trace.point("fetching_original_message");

        let original = sqlx::query!(
            "SELECT guild_id, author_id, author_name, content, created_at FROM message_store WHERE message_id = $1",
            message_id as i64
        )
        .fetch_optional(&*SQL)
        .await
        .map_err(|err| {
            tracing::error!("DB error fetching message: {err:?}");
            CommandError {
                title: String::from("Database error while fetching message"),
                hint: None,
                arg: None,
            }
        })?;

        let Some(original_msg) = original else {
            return Err(CommandError {
                title: String::from("No message found in the database with that ID."),
                hint: None,
                arg: None,
            });
        };

        let guild_id = original_msg.guild_id as u64;

        let is_encrypted = {
            let lock = ENCRYPTION_KEYS.lock().await;
            lock.contains_key(&guild_id)
        };

        let orig_content_bytes = original_msg.content.unwrap_or_default();

        let orig_content = if is_encrypted && !orig_content_bytes.is_empty() {
            let lock = ENCRYPTION_KEYS.lock().await;
            if let Some(key) = lock.get(&guild_id) {
                decrypt(key, &orig_content_bytes)
                    .unwrap_or_else(|| String::from_utf8(orig_content_bytes).unwrap_or_default())
            } else {
                String::from_utf8(orig_content_bytes).unwrap_or_default()
            }
        } else {
            String::from_utf8(orig_content_bytes).unwrap_or_default()
        };

        trace.point("fetching_edits");

        let edits = sqlx::query!(
            "SELECT content, created_at as timestamp FROM message_edits WHERE message_id = $1 ORDER BY created_at ASC",
            message_id as i64
        )
        .fetch_all(&*SQL)
        .await
        .map_err(|err| {
            tracing::error!("DB error fetching edits: {err:?}");
            CommandError {
                title: String::from("Database error while fetching edits"),
                hint: None,
                arg: None,
            }
        })?;

        let mut embeds = Vec::new();
        let mut full_text = String::new();

        let original_embed = CreateEmbed::new()
            .description(format!(
                "**EDIT HISTORY**\n-# ID: `{}` | Author: <@{}>\n```\n{}\n```",
                message_id,
                original_msg.author_id,
                orig_content.replace("```", "\\`\\`\\`")
            ))
            .color(BRAND_BLUE);
        embeds.push(original_embed);

        full_text.push_str(&format!(
            "**EDIT HISTORY**\n-# ID: `{}` | Author: <@{}>\n```\n{}\n```\n",
            message_id,
            original_msg.author_id,
            orig_content.replace("```", "\\`\\`\\`")
        ));

        for edit in edits {
            let edit_content_bytes = edit.content.unwrap_or_default();
            let edit_content = if is_encrypted && !edit_content_bytes.is_empty() {
                let lock = ENCRYPTION_KEYS.lock().await;
                if let Some(key) = lock.get(&guild_id) {
                    decrypt(key, &edit_content_bytes).unwrap_or_else(|| {
                        String::from_utf8(edit_content_bytes).unwrap_or_default()
                    })
                } else {
                    String::from_utf8(edit_content_bytes).unwrap_or_default()
                }
            } else {
                String::from_utf8(edit_content_bytes).unwrap_or_default()
            };

            let edit_embed = CreateEmbed::new()
                .description(format!(
                    "-# At: <t:{0}:d> <t:{0}:T>\n```\n{1}\n```",
                    edit.timestamp.timestamp(),
                    edit_content.replace("```", "\\`\\`\\`")
                ))
                .color(BRAND_BLUE);
            embeds.push(edit_embed);

            full_text.push_str(&format!(
                "-# At: <t:{0}:d> <t:{0}:T>\n```\n{1}\n```\n",
                edit.timestamp.timestamp(),
                edit_content.replace("```", "\\`\\`\\`")
            ));
        }

        let mut reply = CreateMessage::new()
            .reference_message(&msg)
            .allowed_mentions(CreateAllowedMentions::new().replied_user(false));

        if embeds.len() > 10 {
            reply = reply.add_file(serenity::all::CreateAttachment::bytes(
                full_text.as_bytes(),
                "history.txt",
            ));
            reply = reply.embeds(vec![CreateEmbed::new()
                .description(format!(
                    "**EDIT HISTORY**\n-# ID: `{}` | Author: <@{}>\n\nThere are too many edits to display inline. Please check the attached file for the full history.",
                    message_id, original_msg.author_id
                ))
                .color(BRAND_BLUE)]);
        } else {
            reply = reply.embeds(embeds);
        }

        let _ = msg.channel_id.send_message(&ctx, reply).await;

        Ok(())
    }

    fn get_permissions(&self) -> CommandPermissions {
        CommandPermissions::default()
    }
}
