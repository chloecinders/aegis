use chrono::{TimeDelta, Utc};
use serenity::all::{
    Context, CreateEmbed, CreateMessage, GuildId, Member, Mentionable, Permissions, User,
};
use sqlx::query;
use tracing::{error, warn};

use crate::{
    SQL,
    constants::BRAND_BLUE,
    event_handler::CommandError,
    utils::{
        LogType, can_target, guild_log,
        logging::LogContext,
        reference::{RefData, apply_ref_button},
    },
};

pub async fn ban_member(
    ctx: &Context,
    author: Member,
    member: Member,
    guild_id: GuildId,
    db_id: String,
    reason: String,
    clear_days: u8,
    duration: TimeDelta,
    ref_data: RefData,
) -> Result<(), CommandError> {
    let res = can_target(&ctx, &author, &member, Permissions::MODERATE_MEMBERS).await;

    if !res {
        return Err(CommandError {
            title: String::from("You may not target this member."),
            hint: None,
            arg: None,
        });
    }

    return ban_user(
        ctx,
        author,
        member.user,
        guild_id,
        db_id,
        reason,
        clear_days,
        duration,
        ref_data,
    )
    .await;
}

/// Skips the targetting check, so this function should only be used when we are absolutely sure a user isnt in the server/the target check already happened
pub async fn ban_user(
    ctx: &Context,
    author: Member,
    user: User,
    guild_id: GuildId,
    db_id: String,
    mut reason: String,
    clear_days: u8,
    duration: TimeDelta,
    ref_data: RefData,
) -> Result<(), CommandError> {
    if reason.len() > 500 {
        reason.truncate(500);
        reason.push_str("...");
    }

    let disable_past = query!(
        "UPDATE actions SET active = false WHERE guild_id = $1 AND user_id = $2 AND type = 'ban'",
        guild_id.get() as i64,
        user.id.get() as i64,
    )
    .execute(&*SQL);

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

        format!("for {time} {unit}")
    } else {
        String::from("permanent")
    };

    let duration = if duration.is_zero() {
        None
    } else {
        Some((Utc::now() + duration).naive_utc())
    };

    let insert_ban = query!(
        "INSERT INTO actions (id, type, guild_id, user_id, moderator_id, reason, expires_at) VALUES ($1, 'ban', $2, $3, $4, $5, $6)",
        db_id,
        guild_id.get() as i64,
        author.user.id.get() as i64,
        user.id.get() as i64,
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

    if let Err(err) = guild_id
        .ban_with_reason(&ctx, &user, clear_days, &reason)
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

    let mut clear_msg = String::new();

    if clear_days != 0 {
        clear_msg = format!(" | Cleared {clear_days} days of messages");
    }

    let embed = CreateEmbed::new()
        .description(format!(
            "**MEMBER BANNED**\n-# Log ID: `{db_id}` | Actor: {} | Target: {} | Duration: {time_string}{clear_msg}\n```\n{reason}\n```",
            author.mention(),
            user.mention()
        ))
        .color(BRAND_BLUE);

    let msg = apply_ref_button(CreateMessage::new().add_embed(embed), &db_id, &ref_data);

    guild_log(
        &ctx,
        LogType::MemberModeration,
        guild_id,
        msg,
        Some(LogContext {
            target_id: user.id.get(),
            moderator_id: author.user.id.get(),
            db_id: Some(db_id.clone()),
            content: None,
        }),
    )
    .await;

    Ok(())
}
