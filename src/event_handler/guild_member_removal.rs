use chrono::Utc;
use serenity::all::{
    Context, CreateEmbed, CreateMessage, GuildId, Member,
    MemberAction::{BanAdd, Kick},
    Mentionable, User, UserId,
    audit_log::Action,
};

use crate::{
    constants::BRAND_BLUE,
    event_handler::Handler,
    utils::{LogType, guild_log, snowflake_to_timestamp},
};

enum LeaveType {
    User,
    Kick(UserId, String),
    Ban(UserId, String),
}

pub async fn guild_member_removal(
    _handler: &Handler,
    ctx: Context,
    guild_id: GuildId,
    user: User,
    _member_data_if_available: Option<Member>,
) {
    {
        // let mut settings = GUILD_SETTINGS.get().unwrap().lock().await;

        // if let Ok(guild_settings) = settings.get(guild_id.get()).await {
        //     if user.bot && guild_settings.log.log_bots.is_none_or(|b| !b) {
        //         return;
        //     }
        // } else {
        //     warn!("Found guild with no cached settings; Id = {}", guild_id);
        // };

        if user.bot {
            return;
        }
    }

    let audit_log = guild_id
        .audit_logs(&ctx, None, None, None, Some(5))
        .await
        .ok();

    let mut leave_type = LeaveType::User;

    if let Some(logs) = audit_log {
        for entry in logs.entries {
            let entry_time = snowflake_to_timestamp(entry.id.get());

            if (Utc::now() - entry_time).num_seconds().abs() > 5 {
                continue;
            }

            if let Some(target_id) = entry.target_id
                && target_id.get() != user.id.get()
            {
                continue;
            }

            match entry.action {
                Action::Member(Kick) => {
                    if let Some(target) = entry.target_id
                        && user.id.get() == target.get()
                    {
                        leave_type = LeaveType::Kick(
                            entry.user_id,
                            entry.reason.unwrap_or(String::from("No reason provided")),
                        );
                    }
                    break;
                }
                Action::Member(BanAdd) => {
                    if let Some(target) = entry.target_id
                        && user.id.get() == target.get()
                    {
                        leave_type = LeaveType::Ban(
                            entry.user_id,
                            entry.reason.unwrap_or(String::from("No reason provided")),
                        );
                    }
                    break;
                }
                _ => {}
            }
        }
    }

    match leave_type {
        LeaveType::Kick(actor, reason) => {
            if actor.get() == ctx.cache.current_user().id.get() {
                return;
            }

            guild_log(
                &ctx,
                LogType::MemberModeration,
                guild_id,
                CreateMessage::new().add_embed(
                    CreateEmbed::new()
                        .description(format!(
                            "**MEMBER KICKED**\n-# Actor: {} | Target: {}\n```\n{reason}\n```",
                            actor.mention(),
                            user.mention(),
                        ))
                        .color(BRAND_BLUE),
                ),
                Some(crate::utils::logging::LogContext {
                    target_id: user.id.get(),
                    moderator_id: actor.get(),
                    db_id: None,
                    content: None,
                }),
            )
            .await;
        }
        LeaveType::Ban(actor, reason) => {
            if actor.get() == ctx.cache.current_user().id.get() {
                return;
            }

            guild_log(
                &ctx,
                LogType::MemberModeration,
                guild_id,
                CreateMessage::new().add_embed(
                    CreateEmbed::new()
                        .description(format!(
                            "**MEMBER BANNED**\n-# Actor: {} | Target: {}\n```\n{reason}\n```",
                            actor.mention(),
                            user.mention(),
                        ))
                        .color(BRAND_BLUE),
                ),
                Some(crate::utils::logging::LogContext {
                    target_id: user.id.get(),
                    moderator_id: actor.get(),
                    db_id: None,
                    content: None,
                }),
            )
            .await;
        }
        _ => {}
    }
}
