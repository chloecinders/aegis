use serenity::all::{
    Context, CreateEmbed, CreateEmbedAuthor, CreateMessage, Mentionable, VoiceState,
    audit_log::{Action, MemberAction},
};

use crate::{
    constants::{BRAND_BLUE, BRAND_RED, SOFT_GREEN, SOFT_YELLOW},
    event_handler::Handler,
    utils::{LogType, find_audit_log, guild_log},
};

pub async fn voice_state_update(
    _handler: &Handler,
    ctx: Context,
    old: Option<VoiceState>,
    new: VoiceState,
) {
    let guild_id = match new.guild_id {
        Some(id) => id,
        None => return,
    };

    let user = match new.user_id.to_user(&ctx).await {
        Ok(u) => u,
        Err(_) => return,
    };

    if user.bot {
        return;
    }

    let user_mention = new.user_id.mention().to_string();
    let author = CreateEmbedAuthor::new(format!("{}: {}", user.name, user.id.get()))
        .icon_url(user.avatar_url().unwrap_or(user.default_avatar_url()));

    let old_channel = old.as_ref().and_then(|o| o.channel_id);
    let new_channel = new.channel_id;

    match (old_channel, new_channel) {
        (None, Some(ch)) => {
            let embed = CreateEmbed::new()
                .color(SOFT_GREEN)
                .description(format!(
                    "**VOICE JOINED {user_mention}**\n-# Channel: {}",
                    ch.mention()
                ))
                .author(author.clone());

            let msg = CreateMessage::new().add_embed(embed);
            guild_log(&ctx, LogType::VoiceActivity, guild_id, msg, None).await;
            return;
        }
        (Some(_), None) => {
            let left_channel = old_channel
                .map(|c| c.mention().to_string())
                .unwrap_or_else(|| String::from("a voice channel"));

            let mut actor_str = String::new();
            if let Some(log) = find_audit_log(
                &ctx,
                guild_id,
                Action::Member(MemberAction::MemberDisconnect),
                |_| true,
            )
            .await
            {
                actor_str = format!(" | Actor: <@{}>", log.user_id.get());
            }

            let embed = CreateEmbed::new()
                .color(BRAND_RED)
                .description(format!(
                    "**VOICE LEFT {user_mention}**\n-# Channel: {left_channel}{actor_str}"
                ))
                .author(author.clone());

            let msg = CreateMessage::new().add_embed(embed);
            guild_log(&ctx, LogType::VoiceActivity, guild_id, msg, None).await;
            return;
        }
        (Some(old_ch), Some(new_ch)) if old_ch != new_ch => {
            let mut actor_str = String::new();
            if let Some(log) = find_audit_log(
                &ctx,
                guild_id,
                Action::Member(MemberAction::MemberMove),
                |_| true,
            )
            .await
            {
                actor_str = format!(" | Actor: <@{}>", log.user_id.get());
            }

            let embed = CreateEmbed::new()
                .color(SOFT_YELLOW)
                .description(format!(
                    "**VOICE MOVED {user_mention}**\n-# Channel: {} -> {}{actor_str}",
                    old_ch.mention(),
                    new_ch.mention()
                ))
                .author(author.clone());

            let msg = CreateMessage::new().add_embed(embed);
            guild_log(&ctx, LogType::VoiceActivity, guild_id, msg, None).await;
        }
        _ => {}
    }

    let Some(old) = old else { return };

    let mut changes: Vec<String> = Vec::new();
    let mut has_update = false;

    let mut title = String::from("SERVER VOICE UPDATE");
    if old.mute != new.mute && old.deaf == new.deaf {
        has_update = true;
        title = String::from(if new.mute {
            "SERVER MUTED"
        } else {
            "SERVER UNMUTED"
        });
    } else if old.deaf != new.deaf && old.mute == new.mute {
        has_update = true;
        title = String::from(if new.deaf {
            "SERVER DEAFENED"
        } else {
            "SERVER UNDEAFENED"
        });
    } else if old.mute != new.mute || old.deaf != new.deaf {
        has_update = true;
        if old.mute != new.mute {
            changes.push(format!(
                "Server Mute: `{}` -> `{}`",
                bool_label(old.mute),
                bool_label(new.mute)
            ));
        }
        if old.deaf != new.deaf {
            changes.push(format!(
                "Server Deafen: `{}` -> `{}`",
                bool_label(old.deaf),
                bool_label(new.deaf)
            ));
        }
    }

    if !has_update {
        return;
    }

    let channel_mention = new_channel
        .map(|c| format!("Channel: {}", c.mention()))
        .unwrap_or_else(|| String::from("Channel: (unknown)"));

    let mut actor_str = String::new();
    if let Some(log) = find_audit_log(&ctx, guild_id, Action::Member(MemberAction::Update), |e| {
        e.target_id.map(|id| id.get()) == Some(new.user_id.get())
    })
    .await
    {
        actor_str = format!(" | Actor: <@{}>", log.user_id.get());
    }

    let description = if changes.is_empty() {
        format!("**{title} {user_mention}**\n-# {channel_mention}{actor_str}")
    } else {
        format!(
            "**{title} {user_mention}**\n-# {channel_mention}{actor_str}\n{}",
            changes.join("\n")
        )
    };

    let embed = CreateEmbed::new()
        .color(BRAND_BLUE)
        .description(description)
        .author(author);

    let msg = CreateMessage::new().add_embed(embed);
    guild_log(&ctx, LogType::VoiceActivity, guild_id, msg, None).await;
}

fn bool_label(v: bool) -> &'static str {
    if v { "yes" } else { "no" }
}
