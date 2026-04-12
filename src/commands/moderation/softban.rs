use std::sync::Arc;

use serenity::{
    all::{Context, GuildId, Mentionable, Message, Permissions},
    async_trait,
};

use crate::{
    commands::{
        Command, CommandArgument, CommandCategory, CommandParameter, CommandPermissions,
        CommandSyntax, TransformerFnArc,
    },
    event_handler::{CommandError, Handler},
    lexer::{InferType, Token},
    moderation,
    transformers::Transformers,
    utils::{CommandMessageResponse, can_target, tinyid},
};
use ouroboros_macros::command;

pub struct Softban;

impl Softban {
    pub fn new() -> Self {
        Self {}
    }
}

#[async_trait]
impl Command for Softban {
    fn get_name(&self) -> &'static str {
        "softban"
    }

    fn get_short(&self) -> &'static str {
        "Softbans a member from the server"
    }

    fn get_full(&self) -> &'static str {
        "Bans and immediately unbans a member from the server and leaves a note in the users log. \
        Useful for clearing out messages without permanent consequences. \
        Clears 1 day of messages."
    }

    fn get_syntax(&self) -> Vec<CommandSyntax> {
        vec![
            CommandSyntax::Member("user", true),
            CommandSyntax::Consume("reason"),
        ]
    }

    fn get_category(&self) -> CommandCategory {
        CommandCategory::Moderation
    }

    fn get_params(&self) -> Vec<&'static CommandParameter<'static>> {
        vec![
            &CommandParameter {
                name: "clear",
                short: "c",
                transformer: &Transformers::i32,
                desc: "Amount of messages to clear (in days 0-7)",
            },
            &CommandParameter {
                name: "silent",
                short: "s",
                transformer: &Transformers::none,
                desc: "Disables DMing the target with the reason",
            },
        ]
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
        let reason = reason
            .map(|s| {
                if s.is_empty() || s.chars().all(char::is_whitespace) {
                    String::from("No reason provided")
                } else {
                    s
                }
            })
            .unwrap_or(String::from("No reason provided"));

        let days = {
            if let Some(arg) = params.get("clear") {
                if !arg.0 {
                    0
                } else if let CommandArgument::i32(days) = arg.1 {
                    days.clamp(0, 7) as u8
                } else {
                    1
                }
            } else {
                1
            }
        };

        if inferred && let Some(reply) = msg.referenced_message.clone() {
            let _ = reply.delete(&ctx).await;
        }

        let db_id = tinyid().await;

        let mut clear_msg = String::new();

        if days != 0 {
            clear_msg = format!(" | Cleared {days} days of messages");
        }

        let guild_name = {
            match msg
                .guild_id
                .unwrap_or(GuildId::new(1))
                .to_partial_guild(&ctx)
                .await
            {
                Ok(p) => p.name.clone(),
                Err(_) => String::from("UNKNOWN_GUILD"),
            }
        };

        let static_response_parts = (
            format!(
                "**{} SOFTBANNED**\n-# Log ID: `{db_id}`{clear_msg}",
                member.mention()
            ),
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

        let Ok(author_member) = msg.member(&ctx).await else {
            return Err(CommandError {
                title: String::from("Unexpected error has occured."),
                hint: Some(String::from("could not get author member")),
                arg: None,
            });
        };

        trace.point("executing_sanctions");
        moderation::softban(
            &ctx,
            author_member,
            member,
            msg.guild_id.unwrap_or(GuildId::new(1)),
            db_id,
            reason,
            days,
        )
        .await?;

        cmd_response.send_response(&ctx, &msg).await;

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
