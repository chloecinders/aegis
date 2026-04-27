use serenity::all::{
    Context, CreateEmbed, CreateMessage, GuildId, Member, Mentionable, Permissions,
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

pub async fn kick_member(
    ctx: &Context,
    author: Member,
    member: Member,
    guild_id: GuildId,
    db_id: String,
    mut reason: String,
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

    if reason.len() > 500 {
        reason.truncate(500);
        reason.push_str("...");
    }

    let res = query!(
        "INSERT INTO actions (id, type, guild_id, user_id, moderator_id, reason) VALUES ($1, 'kick', $2, $3, $4, $5)",
        db_id,
        guild_id.get() as i64,
        member.user.id.get() as i64,
        author.user.id.get() as i64,
        reason.as_str()
    ).execute(&*SQL).await;

    if let Err(err) = res {
        warn!("Got error while kicking; err = {err:?}");
        return Err(CommandError {
            title: String::from("Could not kick member"),
            hint: Some(String::from("please try again later")),
            arg: None,
        });
    }

    if let Err(err) = member.kick_with_reason(&ctx, &reason).await {
        warn!("Got error while kicking; err = {err:?}");

        if query!("DELETE FROM actions WHERE id = $1", db_id)
            .execute(&*SQL)
            .await
            .is_err()
        {
            error!(
                "Got an error while kicking and an error with the database! Stray kick entry in DB & manual action required; id = {db_id}; err = {err:?}"
            );
        }

        return Err(CommandError {
            title: String::from("Could not kick member"),
            hint: Some(String::from(
                "check if the bot has the kick members permission or try again later",
            )),
            arg: None,
        });
    }

    let embed = CreateEmbed::new()
        .description(format!(
            "**MEMBER KICKED**\n-# Log ID: `{db_id}` | Actor: {} | Target: {}\n```\n{reason}\n```",
            author.mention(),
            member.mention()
        ))
        .color(BRAND_BLUE);

    let msg = apply_ref_button(CreateMessage::new().add_embed(embed), &db_id, &ref_data);

    guild_log(
        &ctx,
        LogType::MemberModeration,
        guild_id,
        msg,
        Some(LogContext {
            target_id: member.user.id.get(),
            moderator_id: author.user.id.get(),
            db_id: Some(db_id.clone()),
            content: None,
        }),
    )
    .await;

    Ok(())
}
