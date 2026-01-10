use std::sync::Arc;

use chrono::{Duration, Utc};
use serenity::{
    all::{
        Context, CreateEmbed, CreateMessage, EditMember, GuildId, Mentionable, Message, Permissions,
    },
    async_trait,
};
use sqlx::query;
use tracing::{error, warn};

use crate::{
    SQL,
    commands::{
        Command, CommandArgument, CommandCategory, CommandParameter, CommandPermissions,
        CommandSyntax, TransformerFnArc,
    },
    constants::BRAND_BLUE,
    event_handler::{CommandError, Handler},
    lexer::{InferType, Token},
    transformers::Transformers,
    utils::{CommandMessageResponse, LogType, can_target, guild_log, tinyid},
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
        #[transformers::maybe_duration] duration: Option<Duration>,
        #[transformers::reply_consume] reason: Option<String>,
    ) -> Result<(), CommandError> {
        let Ok(author_member) = msg.member(&ctx).await else {
            return Err(CommandError {
                title: String::from("Unexpected error has occured."),
                hint: Some(String::from("could not get author member")),
                arg: None,
            });
        };

        let res = can_target(&ctx, &author_member, &member, Permissions::MODERATE_MEMBERS).await;

        if !res {
            return Err(CommandError {
                title: String::from("You may not target this member."),
                hint: None,
                arg: None,
            });
        }

        let inferred = args
            .first()
            .map(|a| matches!(a.inferred, Some(InferType::Message)))
            .unwrap_or(false);
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

        let duration = if duration.is_zero() {
            None
        } else {
            Some(Utc::now() + duration)
        };

        let res = query!(
            "INSERT INTO actions (id, type, guild_id, user_id, moderator_id, reason, expires_at, last_reapplied_at) VALUES ($1, 'mute', $2, $3, $4, $5, $6, NOW())",
            db_id,
            msg.guild_id.map(|g| g.get() as i64).unwrap_or(0),
            member.user.id.get() as i64,
            msg.author.id.get() as i64,
            reason.as_str(),
            duration.map(|d| d.naive_utc()),
        ).execute(&*SQL).await;

        if let Err(err) = res {
            warn!("Got error while timing out; err = {err:?}");
            return Err(CommandError {
                title: String::from("Could not time member out"),
                hint: Some(String::from("please try again later")),
                arg: None,
            });
        }

        let audit_reason = format!(
            "Ouroboros Managed Mute: log id `{db_id}`. Please use Ouroboros to unmute to avoid accidental re-application!"
        );

        let edit = if let Some(duration) = duration {
            EditMember::new()
                .audit_log_reason(&reason)
                .disable_communication_until_datetime(duration.into())
        } else {
            EditMember::new()
                .audit_log_reason(audit_reason.as_str())
                .disable_communication_until_datetime((Utc::now() + Duration::days(27)).into())
        };

        if let Err(err) = member.guild_id.edit_member(&ctx, &member, edit).await {
            warn!("Got error while timinng out; err = {err:?}");

            if query!("DELETE FROM actions WHERE id = $1", db_id)
                .execute(&*SQL)
                .await
                .is_err()
            {
                error!(
                    "Got an error while timing out and an error with the database! Stray timeout entry in DB & manual action required; id = {db_id}; err = {err:?}"
                );
            }

            return Err(CommandError {
                title: String::from("Could not time member out"),
                hint: Some(String::from(
                    "check if the bot has the timeout members permission or try again later",
                )),
                arg: None,
            });
        }

        if inferred && let Some(reply) = msg.referenced_message.clone() {
            let _ = reply.delete(&ctx).await;
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
            .mark_silent(params.contains_key("silent"));

        cmd_response.send_dm(&ctx).await;
        cmd_response.send_response(&ctx, &msg).await;

        guild_log(
            &ctx,
            LogType::MemberModeration,
            msg.guild_id.unwrap(),
            CreateMessage::new()
                .add_embed(
                    CreateEmbed::new()
                        .description(format!(
                            "**MEMBER TIMEOUT**\n-# Log ID: `{db_id}` | Actor: {} | Target: {} | Duration: {time_string}\n```\n{reason}\n```",
                            msg.author.mention(),
                            member.mention(),
                        ))
                        .color(BRAND_BLUE)
                )
        ).await;

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
