use std::{sync::Arc, time::Duration};

use crate::{
    commands::{
        Command, CommandArgument, CommandCategory, CommandParameter, CommandPermissions,
        CommandSyntax, TransformerFnArc,
    },
    constants::BRAND_BLUE,
    event_handler::{CommandError, Handler},
    lexer::{InferType, Token},
    transformers::Transformers,
    utils::{
        reference::{embeds_for_ref, resolve_ref, save_ref},
        tinyid,
    },
};
use aegis_macros::command;
use serenity::{
    all::{
        Context, CreateAllowedMentions, CreateEmbed, CreateMessage, Mentionable, Message,
        Permissions,
    },
    async_trait,
};
use tokio::time::sleep;
use tracing::warn;

pub struct Unban;

impl Unban {
    pub fn new() -> Self {
        Self {}
    }
}

#[async_trait]
impl Command for Unban {
    fn get_name(&self) -> &'static str {
        "unban"
    }

    fn get_short(&self) -> &'static str {
        "Unbans a member from the server"
    }

    fn get_full(&self) -> &'static str {
        "Unbans a member from the server."
    }

    fn get_syntax(&self) -> Vec<CommandSyntax> {
        vec![
            CommandSyntax::User("user", true),
            CommandSyntax::String("reason", false),
        ]
    }

    fn get_category(&self) -> CommandCategory {
        CommandCategory::Moderation
    }

    fn get_params(&self) -> Vec<&'static CommandParameter<'static>> {
        vec![&CommandParameter {
            name: "ref",
            short: "r",
            transformer: &Transformers::some_string,
            desc: "A reference link (Discord message URL or image URL)",
        }]
    }

    #[command]
    async fn run(
        &self,
        ctx: Context,
        msg: Message,
        #[transformers::reply_user] user: User,
        #[transformers::reply_consume] reason: Option<String>,
        params: std::collections::HashMap<&str, (bool, CommandArgument)>,
        trace: &mut TraceContext,
    ) -> Result<(), CommandError> {
        let inferred = matches!(_user_arg.inferred, Some(InferType::Message));
        let mut reason = reason
            .map(|s| {
                if s.is_empty() || s.chars().all(char::is_whitespace) {
                    String::from("No reason provided")
                } else {
                    s
                }
            })
            .unwrap_or(String::from("No reason provided"));

        if reason.len() > 500 {
            reason.truncate(500);
            reason.push_str("...");
        }

        let db_id = tinyid().await;
        let ref_url = params.get("ref").and_then(|(active, arg)| {
            if *active {
                if let CommandArgument::String(s) = arg {
                    Some(s.clone())
                } else {
                    None
                }
            } else {
                None
            }
        });
        let reason_is_default = reason == "No reason provided";

        let Ok(author_member) = msg.member(&ctx).await else {
            return Err(CommandError {
                title: String::from("Unexpected error has occured."),
                hint: Some(String::from("could not get author member")),
                arg: None,
            });
        };

        let ref_data = resolve_ref(&ctx, &msg, &db_id, ref_url.as_deref()).await;

        trace.point("executing_sanctions");
        crate::moderation::unban_user(
            &ctx,
            author_member,
            user.clone(),
            msg.guild_id.unwrap_or_default(),
            db_id.clone(),
            reason.clone(),
            ref_data.clone(),
        )
        .await?;

        save_ref(&db_id, &ref_data, reason_is_default).await;

        let mut header_addition = String::new();
        if ref_data.1.is_some() && !ref_data.1.as_ref().unwrap().is_empty() {
            header_addition.push_str(" | + Images");
        }

        let embed = CreateEmbed::new()
            .description(format!(
                "**{} UNBANNED**\n-# Log ID: `{db_id}`{}\n```\n{reason}\n```",
                user.mention(),
                header_addition
            ))
            .color(BRAND_BLUE);

        let mut reply = CreateMessage::new()
            .add_embed(embed)
            .reference_message(&msg)
            .allowed_mentions(CreateAllowedMentions::new().replied_user(false));

        for ref_embed in embeds_for_ref(&ref_data) {
            reply = reply.add_embed(ref_embed);
        }

        let reply_msg = msg.channel_id.send_message(&ctx, reply).await;

        let reply_msg = match reply_msg {
            Ok(m) => m,
            Err(err) => {
                warn!("Could not send message; err = {err:?}");
                return Ok(());
            }
        };

        trace.point("done");
        if inferred && let Some(reply) = msg.referenced_message.clone() {
            if reply.author.id != ctx.cache.current_user().id {
                let _ = reply.delete(&ctx).await;
            }
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
