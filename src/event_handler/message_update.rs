use chrono::Utc;
use serenity::all::{
    Channel, Context, CreateAttachment, CreateEmbed, CreateEmbedAuthor, CreateMessage, Message,
    MessageUpdateEvent,
};

use crate::{
    constants::SOFT_YELLOW,
    event_handler::Handler,
    utils::{LogType, cache::partials::PartialMessage, create_diff, guild_log},
};

pub async fn message_update(
    _handler: &Handler,
    ctx: Context,
    old_if_available: Option<PartialMessage>,
    new: Option<Message>,
    event: MessageUpdateEvent,
) {
    if event
        .edited_timestamp
        .is_none_or(|t| t.timestamp() < Utc::now().timestamp())
    {
        return;
    }

    if let Some(user) = event.author
        && user.bot
    {
        return;
    }

    let mut new_msg = match new {
        Some(m) if m.author.id != ctx.cache.current_user().id => m,
        Some(_) => return,
        None => match event.channel_id.message(&ctx, event.id).await {
            Ok(m) if m.author.id != ctx.cache.current_user().id => m,
            _ => return,
        },
    };

    if new_msg.content.is_empty() {
        new_msg.content = String::from("(no content)");
    }

    let guild_id = {
        if let Some(Some(channel)) = new_msg.channel(&ctx).await.map(|c| c.guild()).ok() {
            channel.guild_id.get()
        } else {
            0
        }
    };

    let base = format!(
        "**MESSAGE EDITED**\n-# ID: {0} [jump](https://discord.com/channels/{3}/{2}/{0}) | Target: <@{1}> | Channel: <#{2}>",
        new_msg.id.get(),
        new_msg.author.id.get(),
        new_msg.channel_id.get(),
        guild_id
    );

    let old_content_saved = old_if_available.as_ref().map(|o| o.content.clone()).filter(|c| !c.is_empty());

    let (desc, file) = match old_if_available {
        Some(mut old) => {
            if old.content.is_empty() {
                old.content = String::from("(no content)");
            }

            if old.content.len() > 500 || new_msg.content.len() > 500 {
                (
                    base.clone(),
                    Some(CreateAttachment::bytes(
                        create_diff(old.content, new_msg.content).as_bytes(),
                        "msg.diff",
                    )),
                )
            } else {
                (
                    format!(
                        "{base}\nBefore:```\n{}\n```\nAfter:\n```\n{}\n```",
                        old.content.replace("```", "\\`\\`\\`"),
                        new_msg.content.replace("```", "\\`\\`\\`"),
                    ),
                    None,
                )
            }
        }
        None => {
            if new_msg.content.len() > 500 {
                (
                    format!("{base}\n-# Previous message content not found in cache"),
                    Some(CreateAttachment::bytes(
                        new_msg.content.as_bytes(),
                        "new.txt",
                    )),
                )
            } else {
                (
                    format!(
                        "{base}\n-# Previous message content not found in cache\nAfter:\n```\n{}\n```",
                        new_msg.content.replace("```", "\\`\\`\\`"),
                    ),
                    None,
                )
            }
        }
    };

    let mut message = CreateMessage::new().add_embed(
        CreateEmbed::new()
            .color(SOFT_YELLOW)
            .description(desc)
            .author(
                CreateEmbedAuthor::new(format!(
                    "{}: {}",
                    new_msg.author.name,
                    new_msg.author.id.get()
                ))
                .icon_url(
                    new_msg
                        .author
                        .avatar_url()
                        .unwrap_or(new_msg.author.default_avatar_url()),
                ),
            ),
    );

    if let Some(f) = file {
        message = message.add_file(f);
    }

    let guild_id = match new_msg.channel_id.to_channel(&ctx).await {
        Ok(Channel::Guild(g)) => g.guild_id.get(),
        _ => new_msg.guild_id.map(|g| g.get()).unwrap_or(1),
    };

    guild_log(&ctx, LogType::MessageUpdate, guild_id.into(), message, Some(
        crate::utils::logging::LogContext {
            target_id: new_msg.author.id.get(),
            moderator_id: new_msg.author.id.get(),
            db_id: None,
            content: old_content_saved,
        }
    )).await;
}
