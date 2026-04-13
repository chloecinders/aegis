use serenity::all::{
    Context, CreateEmbed, CreateMessage, GuildId, Member, Mentionable, Permissions,
};
use sqlx::query;
use tracing::warn;

use crate::{
    SQL,
    constants::BRAND_BLUE,
    event_handler::CommandError,
    utils::{LogType, can_target, guild_log, logging::LogContext},
};

pub async fn warn_member(
    ctx: &Context,
    author: Member,
    member: Member,
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
        "INSERT INTO actions (id, type, guild_id, user_id, moderator_id, reason) VALUES ($1, 'warn', $2, $3, $4, $5)",
        db_id,
        guild_id.get() as i64,
        member.user.id.get() as i64,
        author.user.id.get() as i64,
        reason.as_str()
    ).execute(&*SQL).await;

    if let Err(err) = res {
        warn!("Got error while warning; err = {err:?}");
        return Err(CommandError {
            title: String::from("Could not warn member"),
            hint: Some(String::from("please try again later")),
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
                        "**MEMBER WARNED**\n-# Log ID: `{db_id}` | Actor: {} | Target: {}\n```\n{reason}\n```",
                        author.mention(),
                        member.mention(),
                    ))
                    .color(BRAND_BLUE)
            ),
            Some(LogContext {
                target_id: member.user.id.get(),
                moderator_id: author.user.id.get(),
                db_id: Some(db_id.clone()),
                content: None,
            }),
    ).await;

    Ok(())
}
