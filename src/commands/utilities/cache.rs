use std::{sync::Arc, time::Duration};

use serenity::{
    all::{
        Context, CreateAllowedMentions, CreateEmbed, CreateMessage, Mentionable, Message,
        Permissions,
    },
    async_trait,
};
use tokio::time::sleep;
use tracing::warn;

use crate::{
    commands::{
        Command, CommandArgument, CommandCategory, CommandParameter, CommandPermissions,
        CommandSyntax, TransformerFnArc,
    },
    constants::BRAND_BLUE,
    event_handler::CommandError,
    lexer::{InferType, Token},
    transformers::Transformers,
    utils::{LogType, guild_log},
};
use ouroboros_macros::command;

pub struct Cache;

impl Cache {
    pub fn new() -> Self {
        Self {}
    }
}

#[async_trait]
impl Command for Cache {
    fn get_name(&self) -> &'static str {
        "cache"
    }

    fn get_short(&self) -> &'static str {
        "Causes clients to cache the target user"
    }

    fn get_full(&self) -> &'static str {
        "Bans and immediately unbans a user to make clients cache the user. \
        Does not work on members who are already in the server, as those do not need to be forced into the cache."
    }

    fn get_syntax(&self) -> Vec<CommandSyntax> {
        vec![CommandSyntax::User("user", true)]
    }

    fn get_category(&self) -> CommandCategory {
        CommandCategory::Utilities
    }

    fn get_params(&self) -> Vec<&'static CommandParameter<'static>> {
        vec![]
    }

    #[command]
    async fn run(
        &self,
        ctx: Context,
        msg: Message,
        #[transformers::reply_user] user: User,
    ) -> Result<(), CommandError> {
        let inferred = args
            .first()
            .map(|a| matches!(a.inferred, Some(InferType::Message)))
            .unwrap_or(false);
        if msg.guild_id.unwrap().member(&ctx, user.id).await.is_ok() {
            return Err(CommandError {
                title: String::from("User was found in the server"),
                hint: None,
                arg: None,
            });
        }

        if let Err(err) = msg
            .guild_id
            .unwrap()
            .ban_with_reason(&ctx, &user, 0, "Forced into client cache")
            .await
        {
            warn!("Got error while banning; err = {err:?}");

            return Err(CommandError {
                title: String::from("Could not ban member"),
                hint: Some(String::from(
                    "check if the bot has the ban members permission or try again later",
                )),
                arg: None,
            });
        }

        if let Err(err) = msg.guild_id.unwrap().unban(&ctx, &user).await {
            warn!("Got error while unbanning; err = {err:?}");

            return Err(CommandError {
                title: String::from("Could not unban member"),
                hint: Some(String::from(
                    "check if the bot has the ban members permission or try again later",
                )),
                arg: None,
            });
        }

        let reply = CreateMessage::new()
            .add_embed(
                CreateEmbed::new()
                    .description(format!(
                        "**{0} CACHED**\n-# Target: {0} `{1}`",
                        user.mention(),
                        user.id.get()
                    ))
                    .color(BRAND_BLUE),
            )
            .reference_message(&msg)
            .allowed_mentions(CreateAllowedMentions::new().replied_user(false));

        let reply_msg = msg.channel_id.send_message(&ctx, reply).await;

        guild_log(
            &ctx,
            LogType::MemberModeration,
            msg.guild_id.unwrap(),
            CreateMessage::new().add_embed(
                CreateEmbed::new()
                    .description(format!(
                        "**MEMBER CACHED**\n-# Actor: {} | Target: {}",
                        msg.author.mention(),
                        user.mention(),
                    ))
                    .color(BRAND_BLUE),
            ),
        )
        .await;

        let reply_msg = match reply_msg {
            Ok(m) => m,
            Err(err) => {
                warn!("Could not send message; err = {err:?}");
                return Ok(());
            }
        };

        if inferred && let Some(reply) = msg.referenced_message.clone() {
            let _ = reply.delete(&ctx).await;
        }

        if inferred {
            tokio::spawn(async move {
                sleep(Duration::from_secs(5)).await;
                let _ = msg.delete(&ctx).await;
                let _ = reply_msg.delete(&ctx).await;
            });
        }

        Ok(())
    }

    fn get_permissions(&self) -> CommandPermissions {
        CommandPermissions {
            required: vec![Permissions::BAN_MEMBERS],
            one_of: vec![],
            bot: [
                CommandPermissions::baseline().as_slice(),
                CommandPermissions::moderation().as_slice(),
            ]
            .concat(),
        }
    }
}
