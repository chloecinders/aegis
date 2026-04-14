use std::sync::Arc;

use crate::{
    commands::{
        Command, CommandArgument, CommandCategory, CommandParameter, CommandPermissions,
        CommandSyntax, TransformerFnArc,
    },
    event_handler::{CommandError, Handler},
    lexer::{InferType, Token},
    transformers::Transformers,
    utils::{CommandMessageResponse, can_target, tinyid},
};
use ouroboros_macros::command;
use serenity::{
    all::{Context, Mentionable, Message, Permissions},
    async_trait,
};

pub struct Kick;

impl Kick {
    pub fn new() -> Self {
        Self {}
    }
}

#[async_trait]
impl Command for Kick {
    fn get_name(&self) -> &'static str {
        "kick"
    }

    fn get_short(&self) -> &'static str {
        "Kicks a member from the server"
    }

    fn get_full(&self) -> &'static str {
        "Kicks a member from the server and leaves a note in the users log."
    }

    fn get_syntax(&self) -> Vec<CommandSyntax> {
        vec![
            CommandSyntax::Member("member", true),
            CommandSyntax::Reason("reason"),
        ]
    }

    fn get_category(&self) -> CommandCategory {
        CommandCategory::Moderation
    }

    fn get_params(&self) -> Vec<&'static CommandParameter<'static>> {
        vec![&CommandParameter {
            name: "silent",
            short: "s",
            transformer: &Transformers::none,
            desc: "Disables DMing the target with the reason",
        }]
    }

    #[command]
    async fn run(
        &self,
        ctx: Context,
        msg: Message,
        #[transformers::reply_member] member: Member,
        #[transformers::reply_consume] reason: Option<String>,
        params: std::collections::HashMap<&str, (bool, CommandArgument)>,
        trace: &mut crate::utils::TraceContext,
    ) -> Result<(), CommandError> {
        let guild = crate::utils::get_guild_info(&ctx, msg.guild_id).await;
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

        if inferred && let Some(reply) = msg.referenced_message.clone() {
            if reply.author.id != ctx.cache.current_user().id {
                let _ = reply.delete(&ctx).await;
            }
        }

        let guild_name = guild
            .as_ref()
            .map(|g| g.name())
            .unwrap_or_else(|| String::from("UNKNOWN_GUILD"));

        let static_response_parts = (
            format!("**{} KICKED**\n-# Log ID: `{db_id}`", member.mention()),
            format!("\n```\n{reason}\n```"),
        );

        let mut cmd_response = CommandMessageResponse::new(member.user.id)
            .dm_content(format!(
                "**KICKED**\n-# Server: {}\n```\n{}\n```",
                guild_name, reason
            ))
            .server_content(Box::new(move |a| {
                format!("{}{a}{}", static_response_parts.0, static_response_parts.1)
            }))
            .automatically_delete(inferred)
            .mark_silent(params.contains_key("silent"));

        trace.point("sending_dm");
        cmd_response.send_dm(&ctx).await;

        trace.point("executing_sanctions");
        crate::moderation::kick_member(
            &ctx,
            author_member,
            member,
            msg.guild_id.unwrap_or_default(),
            db_id,
            reason,
        )
        .await?;

        cmd_response.send_response(&ctx, &msg, trace).await;

        Ok(())
    }

    fn get_permissions(&self) -> CommandPermissions {
        CommandPermissions {
            required: vec![Permissions::KICK_MEMBERS],
            one_of: vec![],
            bot: [
                CommandPermissions::baseline().as_slice(),
                CommandPermissions::moderation().as_slice(),
            ]
            .concat(),
        }
    }
}
