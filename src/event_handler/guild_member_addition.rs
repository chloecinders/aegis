use serenity::all::{Context, CreateEmbed, CreateMessage, Member, Mentionable};

use crate::{
    constants::SOFT_GREEN,
    event_handler::Handler,
    utils::{LogType, guild_log, logging::LogContext},
};

pub async fn guild_member_addition(_handler: &Handler, ctx: Context, new_member: Member) {
    if new_member.user.bot {
        return;
    }

    let guild_id = new_member.guild_id;

    let created_ts = new_member.user.created_at().unix_timestamp();
    let log_count =
        match sqlx::query("SELECT COUNT(*) FROM actions WHERE user_id = $1 AND guild_id = $2;")
            .bind(new_member.user.id.get() as i64)
            .bind(guild_id.get() as i64)
            .fetch_one(&*crate::SQL)
            .await
        {
            Ok(rec) => sqlx::Row::try_get::<i64, _>(&rec, 0).unwrap_or(0),
            Err(_) => 0,
        };

    let log_str = if log_count > 0 {
        format!("\nPrevious Logs: `{log_count}`")
    } else {
        String::new()
    };

    guild_log(
        &ctx,
        LogType::MemberJoinLeave,
        guild_id,
        CreateMessage::new().add_embed(
            CreateEmbed::new()
                .description(format!(
                    "**MEMBER JOINED**\n-# User: {} | ID: {}\nAccount Age: <t:{created_ts}:R> (<t:{created_ts}:f>){log_str}",
                    new_member.user.mention(),
                    new_member.user.id.get()
                ))
                .color(SOFT_GREEN),
        ),
        Some(LogContext {
            target_id: new_member.user.id.get(),
            moderator_id: 0,
            db_id: None,
            content: None,
        }),
    )
    .await;
}
