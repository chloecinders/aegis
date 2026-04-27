use std::sync::Arc;

use chrono::Duration;
use serenity::{
    all::{CacheHttp, Context, LightMethod, Mentionable, Message, Permissions, Request, Route},
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
    utils::{
        CommandMessageResponse, get_guild_info,
        reference::{resolve_ref, save_ref},
        tinyid,
    },
};
use aegis_macros::command;

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
        #[transformers::reply_user] user: User,
        #[transformers::maybe_duration] duration: Option<Duration>,
        #[transformers::reply_consume] reason: Option<String>,
        params: HashMap<&str, (bool, CommandArgument)>,
        trace: &mut TraceContext,
    ) -> Result<(), CommandError> {
        let guild = get_guild_info(&ctx, msg.guild_id).await;
        let Some(guild_id) = msg.guild_id else {
            return Err(CommandError {
                title: String::from("Unexpected error has occured."),
                hint: Some(String::from("could not get guild id")),
                arg: None,
            });
        };

        trace.point("checking_discord_ban_cache");

        if ctx
            .http()
            .request(Request::new(
                Route::GuildBan {
                    guild_id,
                    user_id: user.id,
                },
                LightMethod::Get,
            ))
            .await
            .is_ok()
        {
            return Err(CommandError {
                title: String::from("User is already banned"),
                hint: None,
                arg: None,
            });
        }

        let Ok(author_member) = msg.member(&ctx).await else {
            return Err(CommandError {
                title: String::from("Unexpected error has occured."),
                hint: Some(String::from("could not get author member")),
                arg: None,
            });
        };

        let mut target_member_opt = ctx
            .cache
            .guild(guild_id)
            .and_then(|g| g.members.get(&user.id).cloned());

        if target_member_opt.is_none() {
            target_member_opt = guild_id.member(&ctx, user.id).await.ok();
        }

        if let Some(target_member) = target_member_opt {
            trace.point("verifying_permissions");
            let res = crate::utils::can_target(
                &ctx,
                &author_member,
                &target_member,
                Permissions::BAN_MEMBERS,
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

        let inferred = matches!(_user_arg.inferred, Some(InferType::Message));
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

        let mut clear_msg = String::new();

        if days != 0 {
            clear_msg = format!(" | Cleared {days} days of messages");
        }

        let guild_name = guild
            .as_ref()
            .map(|g| g.name())
            .unwrap_or_else(|| String::from("UNKNOWN_GUILD"));

        let ref_data = resolve_ref(&ctx, &msg, &db_id, ref_url.as_deref()).await;

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
            .mark_silent(params.contains_key("silent"))
            .ref_data(ref_data.clone());

        trace.point("sending_dm");
        cmd_response.send_dm(&ctx).await;

        trace.point("executing_sanctions");
        if let Ok(target_member) = guild_id.member(&ctx, user.id).await {
            moderation::ban_member(
                &ctx,
                author_member,
                target_member,
                msg.guild_id.unwrap_or_default(),
                db_id.clone(),
                reason.clone(),
                days,
                duration,
                ref_data.clone(),
            )
            .await?;
        } else {
            moderation::ban_user(
                &ctx,
                author_member,
                user,
                msg.guild_id.unwrap_or_default(),
                db_id.clone(),
                reason.clone(),
                days,
                duration,
                ref_data.clone(),
            )
            .await?;
        }

        save_ref(&db_id_for_ref, &ref_data, guild_id.get(), reason_is_default).await;

        let ctx_clone = ctx.clone();
        let msg_clone = msg.clone();

        if inferred && let Some(reply) = msg_clone.referenced_message.clone() {
            if reply.author.id != ctx_clone.cache.current_user().id {
                let _ = reply.delete(ctx_clone.clone()).await;
            }
        }

        cmd_response.send_response(&ctx, &msg, trace).await;

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
