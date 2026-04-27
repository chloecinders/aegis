use std::{sync::Arc, time::Duration};

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
        can_target,
        reference::{resolve_ref, save_ref},
        tinyid,
    },
};

pub struct Unmute;

impl Unmute {
    pub fn new() -> Self {
        Self {}
    }
}

#[async_trait]
impl Command for Unmute {
    fn get_name(&self) -> &'static str {
        "unmute"
    }

    fn get_short(&self) -> &'static str {
        "Unmutes a member in the server"
    }

    fn get_full(&self) -> &'static str {
        "Unmutes a member in the server."
    }

    fn get_syntax(&self) -> Vec<CommandSyntax> {
        vec![
            CommandSyntax::Member("member", true),
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
        #[transformers::reply_member] member: Member,
        #[transformers::reply_consume] reason: Option<String>,
        params: HashMap<&str, (bool, CommandArgument)>,
        trace: &mut TraceContext,
    ) -> Result<(), CommandError> {
        let Ok(author_member) = msg.member(&ctx).await else {
            return Err(CommandError {
                title: String::from("Unexpected error has occured."),
                hint: Some(String::from("could not get author member")),
                arg: None,
            });
        };

        trace.point("verifying_permissions");
        let res = can_target(&ctx, &author_member, &member, Permissions::MODERATE_MEMBERS).await;

        if !res {
            return Err(CommandError {
                title: String::from("You may not target this member."),
                hint: None,
                arg: None,
            });
        }

        let inferred = matches!(_member_arg.inferred, Some(InferType::Message));
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

        let ref_data = resolve_ref(&ctx, &msg, &db_id, ref_url.as_deref()).await;

        trace.point("executing_sanctions");
        crate::moderation::unmute_member(
            &ctx,
            author_member,
            member.clone(),
            msg.guild_id.unwrap_or_default(),
            db_id.clone(),
            reason.clone(),
            ref_data.clone(),
        )
        .await?;

        save_ref(
            &db_id,
            &ref_data,
            msg.guild_id.map(|g| g.get()).unwrap_or(0),
            reason_is_default,
        )
        .await;

        let mut header_addition = String::new();
        let has_content = ref_data.content.is_some();
        let has_image = ref_data.image_url.is_some();

        if has_content && has_image {
            header_addition.push_str(" | + ref, + image");
        } else if has_content {
            header_addition.push_str(" | + ref");
        } else if has_image {
            header_addition.push_str(" | + image");
        }

        let embed = CreateEmbed::new()
            .description(format!(
                "**{} UNMUTED**\n-# Log ID: `{db_id}`{}\n```\n{reason}\n```",
                member.mention(),
                header_addition
            ))
            .color(BRAND_BLUE);

        let reply = CreateMessage::new()
            .add_embed(embed)
            .reference_message(&msg)
            .allowed_mentions(CreateAllowedMentions::new().replied_user(false));

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
            required: vec![Permissions::MODERATE_MEMBERS],
            one_of: vec![],
            bot: [
                CommandPermissions::baseline().as_slice(),
                CommandPermissions::moderation().as_slice(),
            ]
            .concat(),
        }
    }
}
