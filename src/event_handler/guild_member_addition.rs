use serenity::all::{Context, CreateEmbed, CreateMessage, Member, Mentionable};

use crate::{
    constants::SOFT_GREEN,
    event_handler::Handler,
    utils::{guild_log, LogType},
};

pub async fn guild_member_addition(
    _handler: &Handler,
    ctx: Context,
    new_member: Member,
) {
    if new_member.user.bot {
        return;
    }

    let guild_id = new_member.guild_id;

    guild_log(
        &ctx,
        LogType::MemberJoinLeave,
        guild_id,
        CreateMessage::new().add_embed(
            CreateEmbed::new()
                .description(format!(
                    "**MEMBER JOINED**\n-# User: {} | ID: {}",
                    new_member.user.mention(),
                    new_member.user.id.get()
                ))
                .color(SOFT_GREEN),
        ),
        Some(crate::utils::logging::LogContext {
            target_id: new_member.user.id.get(),
            moderator_id: 0,
            db_id: None,
            content: None,
        }),
    )
    .await;
}
