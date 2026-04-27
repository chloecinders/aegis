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

pub struct Ref;

impl Ref {
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
impl Command for Ref {
    fn get_name(&self) -> &'static str {
        "ref"
    }

    fn get_short(&self) -> &'static str {
        "Displays the reference saved for a moderation action"
    }

    fn get_full(&self) -> &'static str {
        "Displays the saved reference (Discord message content and/or image) \
        for a moderation action. Provide the log ID or reply to a log message."
    }

    fn get_syntax(&self) -> Vec<CommandSyntax> {
        vec![CommandSyntax::String("id", false)]
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
        #[transformers::string] id: Option<String>,
        trace: &mut TraceContext,
    ) -> Result<(), CommandError> {
        let action_id = if let Some(id) = id {
            id
        } else if let Some(id) = Ref::db_id_from_reply(&msg).await {
            id
        } else {
            return Err(CommandError {
                title: String::from("No action ID provided"),
                hint: Some(String::from("provide an ID or reply to a log message")),
                arg: None,
            });
        };

        trace.point("fetching_ref");
        let guild_id = msg.guild_id.map(|g| g.get()).unwrap_or(0);
        let Some(ref_data) = reference::get_ref(&action_id, guild_id).await else {
            let reply = CreateMessage::new()
                .add_embed(
                    CreateEmbed::new()
                        .description(format!(
                            "**No reference saved for `{action_id}`**\n\
                            -# Use `--ref <url>` when running a moderation command to save one."
                        ))
                        .color(BRAND_BLUE),
                )
                .reference_message(&msg)
                .allowed_mentions(CreateAllowedMentions::new().replied_user(false));
            if let Err(err) = msg.channel_id.send_message(&ctx, reply).await {
                warn!("ref: could not send message; err = {err:?}");
            }
            return Ok(());
        };

        let mut embed = CreateEmbed::new().color(BRAND_BLUE);

        let mut description = format!("**Reference for `{action_id}`**");

        if let Some(header) = ref_data.header() {
            description.push('\n');
            description.push_str(&header);
        }

        if let Some(ref content) = ref_data.content {
            description.push_str(&format!("\n```\n{content}\n```"));
        }

        embed = embed.description(description);

        if let Some(ref url) = ref_data.image_url {
            embed = embed.image(url);
        }

        if ref_data.is_empty() {
            embed = embed.description(format!("**No reference data for `{action_id}`**"));
        }

        let reply = CreateMessage::new()
            .add_embed(embed)
            .reference_message(&msg)
            .allowed_mentions(CreateAllowedMentions::new().replied_user(false));

        if let Err(err) = msg.channel_id.send_message(&ctx, reply).await {
            warn!("ref: could not send message; err = {err:?}");
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
