use chrono::Utc;
use serenity::all::{
    Channel, Context, CreateAttachment, CreateEmbed, CreateEmbedAuthor, CreateMessage,
    MessageAction, audit_log::Action,
};

use crate::{
    constants::BRAND_RED,
    event_handler::{Handler, MessageDeleteEvent},
    utils::{LogType, cache::partials::PartialMessage, guild_log, snowflake_to_timestamp},
};

pub async fn message_delete(
    _handler: &Handler,
    ctx: Context,
    event: MessageDeleteEvent,
    old_if_available: Option<PartialMessage>,
) {
    let Some(msg) = old_if_available else {
        return;
    };

    if msg.author.bot {
        return;
    }

    let guild_id = match event.channel_id.to_channel(&ctx).await {
        Ok(Channel::Guild(guild_channel)) => guild_channel.guild_id,
        _ => return,
    };

    let audit_log = guild_id
        .audit_logs(
            &ctx,
            Some(Action::Message(MessageAction::Delete)),
            None,
            None,
            Some(10),
        )
        .await
        .ok();

    let mut actor_id: Option<u64> = None;

    if let Some(logs) = audit_log {
        if let Some(entry) = logs.entries.first() {
            let entry_time = snowflake_to_timestamp(entry.id.get());

            if (Utc::now() - entry_time).num_seconds().abs() <= 5
                && let Some(target) = entry.target_id
                && let Some(Some(channel)) = entry.options.clone().map(|o| o.channel_id)
                && target.get() == msg.author.id
                && channel.get() == msg.channel_id
            {
                actor_id = Some(entry.user_id.get());
            }
        } else {
            for entry in logs.entries {
                let entry_time = snowflake_to_timestamp(entry.id.get());

                if (Utc::now() - entry_time).num_seconds().abs() <= 5
                    && let Some(target) = entry.target_id
                    && let Some(Some(channel)) = entry.options.clone().map(|o| o.channel_id)
                    && target.get() == msg.author.id
                    && channel.get() == msg.channel_id
                {
                    actor_id = Some(entry.user_id.get());
                }
            }
        }
    }

    let mut description = format!("**MESSAGE DELETED**\n-# ID: {0} ", event.message_id.get());
    let mut files = vec![];
    let mut embed = CreateEmbed::new().color(BRAND_RED);

    if let Some(author) = msg.author.to_user(&ctx).await
    {
        description.push_str(&format!("| Target: <@{}> ", msg.author.id));
        embed = embed.author(
            CreateEmbedAuthor::new(format!("{}: {}", msg.author.name, msg.author.id))
                .icon_url(author.avatar_url().unwrap_or(author.default_avatar_url())),
        );

        for attachment in msg.attachment_urls.iter() {
            let name = attachment.name.clone();
            if let Ok(bytes) = attachment.download().await {
                if bytes.len() > 0 {
                    files.push(CreateAttachment::bytes(bytes, name));
                }
            };
        }
    }

    if let Some(moderator) = actor_id {
        description.push_str(&format!("| Actor: <@{moderator}> "));
    };

    description.push_str(&format!("| Channel: <#{0}> ", event.channel_id.get()));
    description.push_str(&format!(
        "\n{}",
        if msg.content.is_empty() {
            String::new()
        } else {
            format!("```\n{} \n```", msg.content)
        }
    ));

    guild_log(
        &ctx,
        LogType::MessageUpdate,
        guild_id,
        CreateMessage::new()
            .add_embed(embed.description(description))
            .add_files(files),
    )
    .await;
}
