use std::sync::Arc;

use chrono::Duration;
use serenity::{
    all::{Context, Mentionable, Message, Permissions},
    async_trait,
};

use crate::{
    commands::{
        Command, CommandArgument, CommandCategory, CommandParameter, CommandPermissions,
        CommandSyntax, TransformerFnArc,
    },
    event_handler::{CommandError, Handler},
    lexer::{InferType, Token},
    transformers::Transformers,
    utils::{
        CommandMessageResponse, can_target,
        reference::{resolve_ref, save_ref},
        tinyid,
    },
};
use ouroboros_macros::command;

pub struct Mute;

impl Mute {
    pub fn new() -> Self {
        Self {}
    }
}

#[async_trait]
impl Command for Mute {
    fn get_name(&self) -> &'static str {
        "mute"
    }

    fn get_short(&self) -> &'static str {
        "Uses the Discord timeout feature on a member"
    }

    fn get_full(&self) -> &'static str {
        "Uses the Discord timeout feature on a member and leaves a note in the users log. \
        Has a max duration of 28 days. Duration (including the removal of the timeout) is managed by Discord"
    }

    fn get_syntax(&self) -> Vec<CommandSyntax> {
        vec![
            CommandSyntax::Member("member", true),
            CommandSyntax::Duration("duration", true),
            CommandSyntax::Reason("reason"),
        ]
    }

    fn get_category(&self) -> CommandCategory {
        CommandCategory::Moderation
    }

    fn get_params(&self) -> Vec<&'static CommandParameter<'static>> {
        vec![
            &CommandParameter {
                name: "silent",
                short: "s",
                transformer: &Transformers::none,
                desc: "Disables DMing the target with the reason",
            },
            &CommandParameter {
                name: "ref",
                short: "r",
                transformer: &Transformers::some_string,
                desc: "A reference link (Discord message URL or image URL)",
            },
        ]
    }

    #[command]
    async fn run(
        &self,
        ctx: Context,
        msg: Message,
        #[transformers::reply_member] member: Member,
        #[transformers::maybe_duration] duration: Option<Duration>,
        #[transformers::reply_consume] reason: Option<String>,
        params: HashMap<&str, (bool, CommandArgument)>,
        trace: &mut TraceContext,
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
        let duration = duration.unwrap_or(Duration::zero());
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
        let reason_is_default = reason == "No reason provided";
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
        let db_id_for_ref = db_id.clone();

        if inferred && let Some(reply) = msg.referenced_message.clone() {
            if reply.author.id != ctx.cache.current_user().id {
                let _ = reply.delete(&ctx).await;
            }
        }

        let guild_name = guild
            .as_ref()
            .map(|g| g.name())
            .unwrap_or_else(|| String::from("UNKNOWN_GUILD"));

        let time_string = if !duration.is_zero() {
            let (time, mut unit) = match () {
                _ if (duration.num_days() as f64 / 365.0).fract() == 0.0
                    && duration.num_days() >= 365 =>
                {
                    (duration.num_days() / 365, String::from("year"))
                }
                _ if (duration.num_days() as f64 / 30.0).fract() == 0.0
                    && duration.num_days() >= 30 =>
                {
                    (duration.num_days() / 30, String::from("month"))
                }
                _ if duration.num_days() != 0 => (duration.num_days(), String::from("day")),
                _ if duration.num_hours() != 0 => (duration.num_hours(), String::from("hour")),
                _ if duration.num_minutes() != 0 => {
                    (duration.num_minutes(), String::from("minute"))
                }
                _ if duration.num_seconds() != 0 => {
                    (duration.num_seconds(), String::from("second"))
                }
                _ => (0, String::new()),
            };

            if time > 1 {
                unit += "s";
            }

            format!("{time} {unit}")
        } else {
            String::from("permanent")
        };

        let ref_data = resolve_ref(&ctx, &msg, &db_id, ref_url.as_deref()).await;

        let static_response_parts = (
            format!(
                "**{} TIMEOUT**\n-# Log ID: `{db_id}` | Duration: {time_string}",
                member.mention()
            ),
            format!("\n```\n{reason}\n```"),
        );

        let mut cmd_response = CommandMessageResponse::new(member.user.id)
            .dm_content(format!(
                "**TIMEOUT**\n-# Server: {} | Duration: {}\n```\n{}\n```",
                guild_name, time_string, reason
            ))
            .server_content(Box::new(move |a| {
                format!("{}{a}{}", static_response_parts.0, static_response_parts.1)
            }))
            .automatically_delete(inferred)
            .mark_silent(params.contains_key("silent"))
            .ref_data(ref_data.clone());

        trace.point("sending_dm");
        cmd_response.send_dm(&ctx).await;

        trace.point("executing_sanctions");
        crate::moderation::mute_member(
            &ctx,
            author_member,
            member,
            msg.guild_id.unwrap_or_default(),
            db_id.clone(),
            reason.clone(),
            duration,
            ref_data.clone(),
        )
        .await?;

        save_ref(&db_id_for_ref, &ref_data, reason_is_default).await;

        cmd_response.send_response(&ctx, &msg, trace).await;

        Ok(())
    }

    fn get_permissions(&self) -> CommandPermissions {
        CommandPermissions {
            required: vec![Permissions::MODERATE_MEMBERS],
            one_of: vec![],
            bot: CommandPermissions::baseline(),
        }
    }
}
