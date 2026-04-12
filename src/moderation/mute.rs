use chrono::{Duration, Utc};
use serenity::all::{
    Context, CreateEmbed, CreateMessage, EditMember, GuildId, Member, Mentionable, Permissions,
};
use sqlx::query;
use tracing::{error, warn};

use crate::{
    SQL,
    constants::BRAND_BLUE,
    event_handler::CommandError,
    utils::{LogType, can_target, guild_log},
};

pub async fn mute_member(
    ctx: &Context,
    author: Member,
    member: Member,
    guild_id: GuildId,
    db_id: String,
    mut reason: String,
    duration: Duration,
) -> Result<(), CommandError> {
    let res = can_target(&ctx, &author, &member, Permissions::MODERATE_MEMBERS).await;

    if !res {
        return Err(CommandError {
            title: String::from("You may not target this member."),
            hint: None,
            arg: None,
        });
    }

    if reason.len() > 500 {
        reason.truncate(500);
        reason.push_str("...");
    }

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
            _ if duration.num_minutes() != 0 => (duration.num_minutes(), String::from("minute")),
            _ if duration.num_seconds() != 0 => (duration.num_seconds(), String::from("second")),
            _ => (0, String::new()),
        };

        if time > 1 {
            unit += "s";
        }

        format!("{time} {unit}")
    } else {
        String::from("permanent")
    };

    let expires_at = if duration.is_zero() {
        None
    } else {
        Some(Utc::now() + duration)
    };

    let res = query!(
        "INSERT INTO actions (id, type, guild_id, user_id, moderator_id, reason, expires_at, last_reapplied_at) VALUES ($1, 'mute', $2, $3, $4, $5, $6, NOW())",
        db_id,
        guild_id.get() as i64,
        member.user.id.get() as i64,
        author.user.id.get() as i64,
        reason.as_str(),
        expires_at.map(|d| d.naive_utc()),
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

    let edit = if let Some(expires_at) = expires_at {
        EditMember::new()
            .audit_log_reason(&reason)
            .disable_communication_until_datetime(expires_at.into())
    } else {
        EditMember::new()
            .audit_log_reason(audit_reason.as_str())
            .disable_communication_until_datetime((Utc::now() + Duration::days(27)).into())
    };

    if let Err(err) = guild_id.edit_member(&ctx, &member, edit).await {
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

    guild_log(
        &ctx,
        LogType::MemberModeration,
        guild_id,
        CreateMessage::new()
            .add_embed(
                CreateEmbed::new()
                    .description(format!(
                        "**MEMBER TIMEOUT**\n-# Log ID: `{db_id}` | Actor: {} | Target: {} | Duration: {time_string}\n```\n{reason}\n```",
                        author.mention(),
                        member.mention(),
                    ))
                    .color(BRAND_BLUE)
            ),
            Some(crate::utils::logging::LogContext {
                target_id: member.user.id.get(),
                moderator_id: author.user.id.get(),
                db_id: Some(db_id.clone()),
            }),
    ).await;

    Ok(())
}
