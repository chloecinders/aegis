use serenity::{
    all::{
        Context, CreateActionRow, CreateAllowedMentions, CreateButton, CreateMessage, Message,
        Permissions,
    },
    async_trait,
};

use crate::{
    ENCRYPTION_KEYS, SQL,
    commands::{
        Command, CommandArgument, CommandCategory, CommandParameter, CommandPermissions,
        CommandSyntax, TransformerFnArc,
    },
    event_handler::{CommandError, Handler},
    lexer::Token,
    utils::{
        consume_pgsql_error,
        encryption::{generate_key, key_to_display},
    },
};
use ouroboros_macros::command;

pub struct Encrypt;

impl Encrypt {
    pub fn new() -> Self {
        Self {}
    }
}

#[async_trait]
impl Command for Encrypt {
    fn get_name(&self) -> &'static str {
        "encrypt"
    }

    fn get_short(&self) -> &'static str {
        "Enables message content encryption"
    }

    fn get_full(&self) -> &'static str {
        "Enables message content encryption for the server. \
        This will encrypt all logged messages before storing their content inside the database. \
        Helps prevent your server messages from getting leaked from potential vulnerabilities. \
        Better safe than sorry! \
        The encryption key will be posted in channel the command was used in, **said channel should be restricted to administrators only**. \
        Make sure the bot can read messages in the channel."
    }

    fn get_syntax(&self) -> Vec<CommandSyntax> {
        vec![]
    }

    fn get_category(&self) -> CommandCategory {
        CommandCategory::Admin
    }

    fn get_params(&self) -> Vec<&'static CommandParameter<'static>> {
        vec![]
    }

    #[command]
    async fn run(
        &self,
        ctx: Context,
        msg: Message,
        _handler: &Handler,
        trace: &mut TraceContext,
    ) -> Result<(), CommandError> {
        let guild_id = msg.guild_id.unwrap().get();

        let is_encrypted = {
            let lock = ENCRYPTION_KEYS.lock().await;
            lock.contains_key(&guild_id)
        };

        if is_encrypted {
            let reply = CreateMessage::new()
                .content(
                    "**ENCRYPTION ENABLED**\nEncryption is already enabled for this server.\n\
                     Click the button below to disable it and permanently wipe all stored messages."
                )
                .components(vec![CreateActionRow::Buttons(vec![
                    CreateButton::new("disable_encryption")
                        .label("Disable Encryption")
                        .style(serenity::all::ButtonStyle::Danger)
                ])])
                .reference_message(&msg)
                .allowed_mentions(CreateAllowedMentions::new().replied_user(false));

            let _ = msg.channel_id.send_message(&ctx, reply).await;
            return Ok(());
        }

        trace.point("generating_key");
        let key = generate_key();
        let key_str = key_to_display(&key);

        trace.point("wiping_old_messages");
        let _ = sqlx::query!("DELETE FROM message_edits WHERE message_id IN (SELECT message_id FROM message_store WHERE guild_id = $1)", guild_id as i64)
            .execute(&*SQL)
            .await;

        let _ = sqlx::query!(
            "DELETE FROM message_store WHERE guild_id = $1",
            guild_id as i64
        )
        .execute(&*SQL)
        .await;

        let embed = serenity::all::CreateEmbed::new()
            .description(format!(
                "**ENCRYPTION ENABLED**\nDO NOT DELETE THIS MESSAGE.\n```\n{}\n```",
                key_str
            ))
            .color(crate::constants::BRAND_BLUE);

        let reply = CreateMessage::new()
            .add_embed(embed)
            .components(vec![CreateActionRow::Buttons(vec![
                CreateButton::new("disable_encryption")
                    .label("Disable Encryption")
                    .style(serenity::all::ButtonStyle::Danger),
            ])])
            .reference_message(&msg)
            .allowed_mentions(CreateAllowedMentions::new().replied_user(false));

        trace.point("sending_key_message");
        let sent_msg = match msg.channel_id.send_message(&ctx, reply).await {
            Ok(m) => m,
            Err(err) => {
                return Err(CommandError {
                    title: format!("Could not send key message: {}", err),
                    hint: None,
                    arg: None,
                });
            }
        };

        trace.point("updating_database");
        let res = sqlx::query!(
            "INSERT INTO guild_encryption (guild_id, encrypted, key_channel_id, key_message_id) VALUES ($1, true, $2, $3)
             ON CONFLICT (guild_id) DO UPDATE SET encrypted = true, key_channel_id = $2, key_message_id = $3",
            guild_id as i64,
            sent_msg.channel_id.get() as i64,
            sent_msg.id.get() as i64
        )
        .execute(&*SQL)
        .await;

        if let Err(err) = res {
            consume_pgsql_error(String::from("ENCRYPT"), err);

            return Err(CommandError {
                title: String::from("Could not enable encryption"),
                hint: Some(String::from("Please try again later")),
                arg: None,
            });
        }

        trace.point("updating_memory");
        {
            let mut keys = ENCRYPTION_KEYS.lock().await;
            keys.insert(guild_id, key);
        }

        Ok(())
    }

    fn get_permissions(&self) -> CommandPermissions {
        CommandPermissions {
            required: vec![Permissions::ADMINISTRATOR],
            one_of: vec![],
            bot: CommandPermissions::baseline(),
        }
    }
}
