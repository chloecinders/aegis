use serenity::all::{
    Context, CreateEmbed, CreateMessage, GuildId, Member, Mentionable, Permissions,
};
use sqlx::query;
use tracing::{error, warn};

use crate::{
    SQL,
    constants::BRAND_BLUE,
    event_handler::CommandError,
    utils::{LogType, can_target, guild_log},
};

pub async fn unmute_member(
    ctx: &Context,
    author: Member,
    mut member: Member,
    guild_id: GuildId,
    db_id: String,
    mut reason: String,
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

    let res = query!(
        "UPDATE actions SET active = false, expires_at = NULL WHERE guild_id = $1 AND user_id = $2 AND type = 'mute' AND active = true;",
        guild_id.get() as i64,
        member.user.id.get() as i64,
    ).execute(&*SQL).await;

    if let Err(err) = res {
        warn!("Got error while unmuting; err = {err:?}");
        return Err(CommandError {
            title: String::from("Could not unmute member"),
            hint: Some(String::from("please try again later")),
            arg: None,
        });
    }

    let res = query!(
        "INSERT INTO actions (id, type, guild_id, user_id, moderator_id, reason) VALUES ($1, 'unmute', $2, $3, $4, $5)",
        db_id,
        guild_id.get() as i64,
        member.user.id.get() as i64,
        author.user.id.get() as i64,
        reason.as_str()
    ).execute(&*SQL).await;

    if let Err(err) = res {
        warn!("Got error while unmuting; err = {err:?}");
        return Err(CommandError {
            title: String::from("Could not unmute member"),
            hint: Some(String::from("please try again later")),
            arg: None,
        });
    }

    if let Err(err) = member.enable_communication(&ctx).await {
        warn!("Got error while unmuting; err = {err:?}");

        if query!("DELETE FROM actions WHERE id = $1", db_id)
            .execute(&*SQL)
            .await
            .is_err()
        {
            error!(
                "Got an error while unmuting and an error with the database! Stray unmute entry in DB & manual action required; id = {db_id}; err = {err:?}"
            );
        }

        return Err(CommandError {
            title: String::from("Could not unmute member"),
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
                        "**MEMBER UNMUTED**\n-# Log ID: `{db_id}` | Actor: {} | Target: {}\n```\n{reason}\n```",
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
