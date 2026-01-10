use std::sync::Arc;

use chrono::{Duration, Utc};
use serenity::{
    all::{Context, CreateEmbed, CreateMessage, GuildId, Mentionable, Message, Permissions},
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

pub struct Ban;

impl Ban {
    pub fn new() -> Self {
        Self {}
    }
}

#[async_trait]
impl Command for Ban {
    fn get_name(&self) -> &'static str {
        "ban"
    }

    fn get_short(&self) -> &'static str {
        "Bans a member from the server and deletes their messages"
    }

    fn get_full(&self) -> &'static str {
        "Bans from the server and leaves a note in the users log. \
        Defaults to permanent if no duration is provided. \
        Use 0 for the duration to make the ban permanent. \
        If the duration cannot be resolved it will default to permanent. \
        Ban expiry is checked every 5 minutes. \
        Clears one day of messages by default."
    }

    fn get_syntax(&self) -> Vec<CommandSyntax> {
        vec![
            CommandSyntax::Member("member", true),
            CommandSyntax::Duration("duration", false),
            CommandSyntax::Number("days", false),
            CommandSyntax::Reason("reason"),
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
        #[transformers::reply_user] user: User,
        #[transformers::maybe_duration] duration: Option<Duration>,
        #[transformers::reply_consume] reason: Option<String>,
    ) -> Result<(), CommandError> {
        let Some(guild_id) = msg.guild_id else {
            return Err(CommandError {
                title: String::from("Unexpected error has occured."),
                hint: Some(String::from("could not get guild id")),
                arg: None,
            });
        };

        if let Ok(target_member) = guild_id.member(&ctx, user.id).await {
            let Ok(author_member) = msg.member(&ctx).await else {
                return Err(CommandError {
                    title: String::from("Unexpected error has occured."),
                    hint: Some(String::from("could not get author member")),
                    arg: None,
                });
            };

            let res = can_target(
                &ctx,
                &author_member,
                &target_member,
                Permissions::MODERATE_MEMBERS,
            )
            .await;
            if !res {
                return Err(CommandError {
                    title: String::from("You may not target this member."),
                    hint: None,
                    arg: None,
                });
            }
        }

        let inferred = args
            .first()
            .map(|a| matches!(a.inferred, Some(InferType::Message)))
            .unwrap_or(false);
        let duration = duration.unwrap_or(Duration::zero());
        let mut reason = reason
            .map(|s| {
                if s.is_empty() || s.chars().all(char::is_whitespace) {
                    String::new()
                } else {
                    s
                }
            })
            .unwrap_or_default();

        if reason.is_empty() {
            reason = String::from("No reason provided")
        }

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

            format!("for {time} {unit}")
        } else {
            String::from("permanent")
        };

        let duration = if duration.is_zero() {
            None
        } else {
            Some((Utc::now() + duration).naive_utc())
        };

        let disable_past = query!(
            "UPDATE actions SET active = false WHERE guild_id = $1 AND user_id = $2 AND type = 'ban'",
            msg.guild_id.map(|g| g.get() as i64).unwrap_or(0),
            user.id.get() as i64,
        ).execute(&*SQL);

        let insert_ban = query!(
            "INSERT INTO actions (id, type, guild_id, user_id, moderator_id, reason, expires_at) VALUES ($1, 'ban', $2, $3, $4, $5, $6)",
            db_id,
            msg.guild_id.map(|g| g.get() as i64).unwrap_or(0),
            user.id.get() as i64,
            msg.author.id.get() as i64,
            reason.as_str(),
            duration
        ).execute(&*SQL);

        let (res1, res2) = tokio::join!(disable_past, insert_ban);

        if let Err(err) = res1 {
            warn!("Got error while banning; err = {err:?}");
            return Err(CommandError {
                title: String::from("Could not ban member"),
                hint: Some(String::from("please try again later")),
                arg: None,
            });
        }

        if let Err(err) = res2 {
            warn!("Got error while banning; err = {err:?}");
            return Err(CommandError {
                title: String::from("Could not ban member"),
                hint: Some(String::from("please try again later")),
                arg: None,
            });
        }

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

        let static_server_contents = (
            format!(
                "**{} BANNED**\n-# Log ID: `{db_id}` | Duration: {time_string}{clear_msg}",
                user.mention()
            ),
            format!("\n```\n{reason}\n```"),
        );

        let mut cmd_response = CommandMessageResponse::new(user.id)
            .dm_content(format!(
                "**BANNED**\n-# Server: {} | Duration: {}\n```\n{}\n```",
                guild_name, time_string, reason
            ))
            .server_content(Box::new(move |a| {
                format!(
                    "{}{a}{}",
                    static_server_contents.0, static_server_contents.1
                )
            }))
            .automatically_delete(inferred)
            .mark_silent(params.contains_key("silent"));

        cmd_response.send_dm(&ctx).await;

        if let Err(err) = msg
            .guild_id
            .unwrap()
            .ban_with_reason(&ctx, &user, days, &reason)
            .await
        {
            warn!("Got error while banning; err = {err:?}");

            if query!("DELETE FROM actions WHERE id = $1", db_id)
                .execute(&*SQL)
                .await
                .is_err()
            {
                error!(
                    "Got an error while banning and an error with the database! Stray ban entry in DB & manual action required; id = {db_id}; err = {err:?}"
                );
            }

            return Err(CommandError {
                title: String::from("Could not ban member"),
                hint: Some(String::from(
                    "check if the bot has the ban members permission or try again later",
                )),
                arg: None,
            });
        }

        let ctx_clone = ctx.clone();
        let msg_clone = msg.clone();

        let delete_user_msg = async move {
            if inferred && let Some(reply) = msg_clone.referenced_message.clone() {
                let _ = reply.delete(ctx_clone.clone()).await;
            }
        };

        cmd_response.send_response(&ctx, &msg).await;

        let send_log = guild_log(
            &ctx,
            LogType::MemberModeration,
            msg.guild_id.unwrap(),
            CreateMessage::new()
                .add_embed(
                    CreateEmbed::new()
                        .description(format!(
                            "**MEMBER BANNED**\n-# Log ID: `{db_id}` | Actor: {} | Target: {} | Duration: {time_string}{clear_msg}\n```\n{reason}\n```",
                            msg.author.mention(),
                            user.mention(),
                        ))
                        .color(BRAND_BLUE)
                )
        );

        tokio::join!(delete_user_msg, send_log);

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
