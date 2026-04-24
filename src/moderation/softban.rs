use serenity::all::{
    Context, CreateEmbed, CreateMessage, GuildId, Member, Mentionable, Permissions,
};
use sqlx::query;
use tracing::{error, warn};

use crate::{
    SQL,
    constants::BRAND_BLUE,
    event_handler::CommandError,
    utils::{LogType, can_target, guild_log, logging::LogContext, reference::apply_ref_button},
};

pub async fn softban(
    ctx: &Context,
    author: Member,
    member: Member,
    guild_id: GuildId,
    db_id: String,
    mut reason: String,
    clear_days: u8,
    ref_data: (Option<String>, Option<String>),
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
            "INSERT INTO actions (id, type, guild_id, user_id, moderator_id, reason) VALUES ($1, 'softban', $2, $3, $4, $5)",
            db_id,
            guild_id.get() as i64,
            member.user.id.get() as i64,
            author.user.id.get() as i64,
            reason.as_str()
        ).execute(&*SQL).await;

    if let Err(err) = res {
        warn!("Got error while softbanning; err = {err:?}");
        return Err(CommandError {
            title: String::from("Could not softban member"),
            hint: Some(String::from("please try again later")),
            arg: None,
        });
    }

    if let Err(err) = member.ban_with_reason(&ctx, clear_days, &reason).await {
        warn!("Got error while softbanning; err = {err:?}");

        if query!("DELETE FROM actions WHERE id = $1", db_id)
            .execute(&*SQL)
            .await
            .is_err()
        {
            error!(
                "Got an error while softbanning and an error with the database! Stray softban entry in DB & manual action required; id = {db_id}; err = {err:?}"
            );
        }

        return Err(CommandError {
            title: String::from("Could not softban member"),
            hint: Some(String::from(
                "check if the bot has the ban members permission or try again later",
            )),
            arg: None,
        });
    }

    if let Err(err) = member.unban(&ctx).await {
        warn!("Got error while softunbanning; err = {err:?}");

        // leave the entry in the db since they have still faced the consequences
        return Err(CommandError {
            title: String::from("Member banned, but bot ran into an error trying to unban"),
            hint: Some(String::from(
                "manually unban the member and check if the bot has the ban members permission",
            )),
            arg: None,
        });
    }

    let embed = CreateEmbed::new()
        .description(format!(
            "**MEMBER SOFTBANNED**\n-# Log ID: `{db_id}` | Actor: {} | Target: {}\n```\n{reason}\n```",
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
