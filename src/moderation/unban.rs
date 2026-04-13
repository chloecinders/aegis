use serenity::all::{Context, CreateEmbed, CreateMessage, GuildId, Member, Mentionable, User};
use sqlx::query;
use tracing::{error, warn};

use crate::{
    SQL,
    constants::BRAND_BLUE,
    event_handler::CommandError,
    utils::{LogType, guild_log, logging::LogContext},
};

pub async fn unban_user(
    ctx: &Context,
    author: Member,
    user: User,
    guild_id: GuildId,
    db_id: String,
    mut reason: String,
) -> Result<(), CommandError> {
    if reason.len() > 500 {
        reason.truncate(500);
        reason.push_str("...");
    }

    let res = query!(
        "UPDATE actions SET active = false, expires_at = NULL WHERE guild_id = $1 AND user_id = $2 AND type = 'ban' AND active = true;",
        guild_id.get() as i64,
        user.id.get() as i64,
    ).execute(&*SQL).await;

    if let Err(err) = res {
        warn!("Got error while unbanning; err = {err:?}");
        return Err(CommandError {
            title: String::from("Could not unban member"),
            hint: Some(String::from("please try again later")),
            arg: None,
        });
    }

    let res = query!(
        "INSERT INTO actions (id, type, guild_id, user_id, moderator_id, reason) VALUES ($1, 'unban', $2, $3, $4, $5)",
        db_id,
        guild_id.get() as i64,
        user.id.get() as i64,
        author.user.id.get() as i64,
        reason.as_str()
    ).execute(&*SQL).await;

    if let Err(err) = res {
        warn!("Got error while unbanning; err = {err:?}");
        return Err(CommandError {
            title: String::from("Could not unban member"),
            hint: Some(String::from("please try again later")),
            arg: None,
        });
    }

    if let Err(err) = ctx.http.remove_ban(guild_id, user.id, Some(&reason)).await {
        warn!("Got error while unbanning; err = {err:?}");

        if query!("DELETE FROM actions WHERE id = $1", db_id)
            .execute(&*SQL)
            .await
            .is_err()
        {
            error!(
                "Got an error while unbanning and an error with the database! Stray unban entry in DB & manual action required; id = {db_id}; err = {err:?}"
            );
        }

        return Err(CommandError {
            title: String::from("Could not unban member"),
            hint: Some(String::from(
                "check if the bot has the ban members permission or try again later",
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
                        "**MEMBER UNBANNED**\n-# Log ID: `{db_id}` | Actor: {} | Target: {}\n```\n{reason}\n```",
                        author.mention(),
                        user.mention(),
                    ))
                    .color(BRAND_BLUE)
            ),
            Some(LogContext {
                target_id: user.id.get(),
                moderator_id: author.user.id.get(),
                db_id: Some(db_id.clone()),
                content: None,
            }),
    ).await;

    Ok(())
}
