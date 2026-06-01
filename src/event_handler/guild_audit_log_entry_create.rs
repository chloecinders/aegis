use serenity::all::{
    AuditLogEntry, ChannelAction, Context, CreateEmbed, CreateMessage, GuildId, Mentionable,
    RoleAction, StickerAction,
    audit_log::{Action, Change, EmojiAction, VoiceChannelStatusAction},
};

use crate::{
    constants::{BRAND_RED, SOFT_GREEN, SOFT_YELLOW},
    event_handler::Handler,
    utils::{LogType, guild_log},
};

pub async fn guild_audit_log_entry_create(
    _handler: &Handler,
    ctx: Context,
    entry: AuditLogEntry,
    guild_id: GuildId,
) {
    match entry.action {
        Action::Channel(action) => handle_channel_action(ctx, entry, guild_id, action).await,
        Action::Role(action) => handle_role_action(ctx, entry, guild_id, action).await,
        Action::VoiceChannelStatus(action) => {
            handle_voice_channel_status_action(ctx, entry, guild_id, action).await
        }
        Action::Emoji(action) => handle_emoji_action(ctx, entry, guild_id, action).await,
        Action::Sticker(action) => handle_sticker_action(ctx, entry, guild_id, action).await,
        _ => {}
    }
}

async fn handle_channel_action(
    ctx: Context,
    entry: AuditLogEntry,
    guild_id: GuildId,
    action: ChannelAction,
) {
    let actor = format!("<@{}>", entry.user_id.get());
    let channel_id = entry.target_id.map(|id| id.get());

    let (label, color) = match action {
        ChannelAction::Create => ("CHANNEL CREATED", SOFT_GREEN),
        ChannelAction::Update => ("CHANNEL UPDATED", SOFT_YELLOW),
        ChannelAction::Delete => ("CHANNEL DELETED", BRAND_RED),
        _ => return,
    };

    let channel_mention = channel_id
        .map(|id| serenity::all::ChannelId::new(id).mention().to_string())
        .unwrap_or_else(|| String::from("(unknown)"));

    let mut description = format!("**{label} {channel_mention}**\n-# Actor: {actor}");

    if matches!(action, ChannelAction::Update) {
        if let Some(changes) = &entry.changes {
            let diff = format_channel_changes(changes);
            if !diff.is_empty() {
                description.push_str(&format!("\n{diff}"));
            }
        }
    }

    if let Some(reason) = &entry.reason {
        description.push_str(&format!("\nReason:\n```{reason} ```"));
    }

    let embed = CreateEmbed::new().color(color).description(description);
    let msg = CreateMessage::new().add_embed(embed);
    guild_log(&ctx, LogType::Channels, guild_id, msg, None).await;
}

fn format_channel_changes(changes: &[Change]) -> String {
    let mut lines = Vec::new();

    for change in changes {
        match change {
            Change::Name { old, new } => lines.push(field_diff("Name", old.clone(), new.clone())),
            Change::Topic { old, new } => lines.push(field_diff("Topic", old.clone(), new.clone())),
            Change::Nsfw { old, new } => {
                lines.push(field_diff("NSFW", opt_bool(old), opt_bool(new)))
            }
            Change::Bitrate { old, new } => lines.push(field_diff(
                "Bitrate",
                old.map(|v| format!("{v}bps")),
                new.map(|v| format!("{v}bps")),
            )),
            Change::RateLimitPerUser { old, new } => lines.push(field_diff(
                "Slowmode",
                old.map(|v| format!("{v}s")),
                new.map(|v| format!("{v}s")),
            )),
            Change::UserLimit { old, new } => lines.push(field_diff(
                "User Limit",
                old.map(|v| v.to_string()),
                new.map(|v| v.to_string()),
            )),
            Change::Position { old, new } => lines.push(field_diff(
                "Position",
                old.map(|v| v.to_string()),
                new.map(|v| v.to_string()),
            )),
            _ => {}
        }
    }

    lines.join("\n")
}

async fn handle_role_action(
    ctx: Context,
    entry: AuditLogEntry,
    guild_id: GuildId,
    action: RoleAction,
) {
    let actor = format!("<@{}>", entry.user_id.get());
    let role_id = entry.target_id.map(|id| id.get());

    let (label, color) = match action {
        RoleAction::Create => ("ROLE CREATED", SOFT_GREEN),
        RoleAction::Update => ("ROLE UPDATED", SOFT_YELLOW),
        RoleAction::Delete => ("ROLE DELETED", BRAND_RED),
        _ => return,
    };

    let role_mention = role_id
        .map(|id| serenity::all::RoleId::new(id).mention().to_string())
        .unwrap_or_else(|| String::from("(unknown)"));

    let mut description = format!("**{label} {role_mention}**\n-# Actor: {actor}");

    if matches!(action, RoleAction::Update) {
        if let Some(changes) = &entry.changes {
            let diff = format_role_changes(changes);
            if !diff.is_empty() {
                description.push_str(&format!("\n{diff}"));
            }
        }
    }

    if let Some(reason) = &entry.reason {
        description.push_str(&format!("\nReason:\n```{reason} ```"));
    }

    let embed = CreateEmbed::new().color(color).description(description);
    let msg = CreateMessage::new().add_embed(embed);
    guild_log(&ctx, LogType::Roles, guild_id, msg, None).await;
}

fn format_role_changes(changes: &[Change]) -> String {
    let mut lines = Vec::new();

    for change in changes {
        match change {
            Change::Name { old, new } => lines.push(field_diff("Name", old.clone(), new.clone())),
            Change::Color { old, new } => lines.push(field_diff(
                "Color",
                old.map(|v| format!("#{v:06X}")),
                new.map(|v| format!("#{v:06X}")),
            )),
            Change::Hoist { old, new } => {
                lines.push(field_diff("Hoisted", opt_bool(old), opt_bool(new)))
            }
            Change::Mentionable { old, new } => {
                lines.push(field_diff("Mentionable", opt_bool(old), opt_bool(new)))
            }
            Change::Permissions { old, new } => lines.push(field_diff(
                "Permissions",
                old.map(|p| format!("`{p:?}`")),
                new.map(|p| format!("`{p:?}`")),
            )),
            _ => {}
        }
    }

    lines.join("\n")
}

fn field_diff(label: &str, old: Option<String>, new: Option<String>) -> String {
    match (old, new) {
        (Some(o), Some(n)) if o != n => format!("{label}: `{o}` -> `{n}`"),
        (None, Some(n)) => format!("{label}: (none) -> `{n}`"),
        (Some(o), None) => format!("{label}: `{o}` -> (none)"),
        _ => String::new(),
    }
}

fn opt_bool(v: &Option<bool>) -> Option<String> {
    v.map(|b| if b { "yes" } else { "no" }.to_string())
}

async fn handle_voice_channel_status_action(
    ctx: Context,
    entry: AuditLogEntry,
    guild_id: GuildId,
    action: VoiceChannelStatusAction,
) {
    let actor = format!("<@{}>", entry.user_id.get());
    let channel_id = entry.target_id.map(|id| id.get());

    let channel_mention = channel_id
        .map(|id| serenity::all::ChannelId::new(id).mention().to_string())
        .unwrap_or_else(|| String::from("(unknown)"));

    let label = match action {
        VoiceChannelStatusAction::StatusUpdate => "VOICE STATUS SET",
        VoiceChannelStatusAction::StatusDelete => "VOICE STATUS CLEARED",
        _ => return,
    };

    let mut description = format!("**{label}**\n-# Channel: {channel_mention} | Actor: {actor}");

    if matches!(action, VoiceChannelStatusAction::StatusUpdate) {
        if let Some(opts) = entry.options {
            description.push_str(&format!(
                "\nNew Status:\n`{}`",
                opts.status.unwrap_or(String::from("(none)"))
            ));
        }
    }

    if let Some(reason) = &entry.reason {
        description.push_str(&format!("\nReason:\n```{reason} ```"));
    }

    let embed = CreateEmbed::new()
        .color(SOFT_YELLOW)
        .description(description);
    let msg = CreateMessage::new().add_embed(embed);
    guild_log(&ctx, LogType::VoiceActivity, guild_id, msg, None).await;
}

async fn handle_emoji_action(
    ctx: Context,
    entry: AuditLogEntry,
    guild_id: GuildId,
    action: EmojiAction,
) {
    let actor = format!("<@{}>", entry.user_id.get());
    let emoji_id = entry.target_id.map(|id| id.get());

    let (label, color) = match action {
        EmojiAction::Create => ("EMOJI CREATED", SOFT_GREEN),
        EmojiAction::Update => ("EMOJI UPDATED", SOFT_YELLOW),
        EmojiAction::Delete => ("EMOJI DELETED", BRAND_RED),
        _ => return,
    };

    let mut found_name = None;
    if let Some(changes) = &entry.changes {
        found_name = changes.iter().find_map(|c| match c {
            Change::Name { new, old, .. } => new.as_ref().or(old.as_ref()).cloned(),
            _ => None,
        });
    }

    let title_emoji = if let (Some(id), Some(name)) = (emoji_id, &found_name) {
        format!("<:{}:{}>", name, id)
    } else {
        String::new()
    };

    let name_str = if let Some(name) = found_name {
        format!("`{}`", name)
    } else {
        String::from("(unknown)")
    };

    let mut description = format!(
        "**{} {}**\n-# Actor: {}\nName: {}",
        label, title_emoji, actor, name_str
    );

    if let Some(id) = emoji_id {
        description.push_str(&format!(" | ID: `{}`", id));
    }

    if matches!(action, EmojiAction::Update) {
        if let Some(changes) = &entry.changes {
            let diff = format_expression_changes(changes);
            if !diff.is_empty() {
                description.push_str(&format!("\n\n{diff}"));
            }
        }
    }

    if let Some(reason) = &entry.reason {
        description.push_str(&format!("\n\nReason:\n```{reason} ```"));
    }

    let mut embed = CreateEmbed::new().color(color).description(description);

    if !matches!(action, EmojiAction::Update) {
        if let Some(id) = emoji_id {
            embed = embed.image(format!(
                "https://cdn.discordapp.com/emojis/{}.webp?size=128",
                id
            ));
        }
    }

    let msg = CreateMessage::new().add_embed(embed);
    guild_log(&ctx, LogType::Expressions, guild_id, msg, None).await;
}

async fn handle_sticker_action(
    ctx: Context,
    entry: AuditLogEntry,
    guild_id: GuildId,
    action: StickerAction,
) {
    let actor = format!("<@{}>", entry.user_id.get());
    let sticker_id = entry.target_id.map(|id| id.get());

    let (label, color) = match action {
        StickerAction::Create => ("STICKER CREATED", SOFT_GREEN),
        StickerAction::Update => ("STICKER UPDATED", SOFT_YELLOW),
        StickerAction::Delete => ("STICKER DELETED", BRAND_RED),
        _ => return,
    };

    let mut found_name = None;
    if let Some(changes) = &entry.changes {
        found_name = changes.iter().find_map(|c| match c {
            Change::Name { new, old, .. } => new.as_ref().or(old.as_ref()).cloned(),
            _ => None,
        });
    }

    let name_str = if let Some(name) = found_name {
        format!("`{}`", name)
    } else {
        String::from("(unknown)")
    };

    let mut description = format!("**{}**\n-# Actor: {}\nName: {}", label, actor, name_str);

    if let Some(id) = sticker_id {
        description.push_str(&format!(" | ID: `{}`", id));
    }

    if matches!(action, StickerAction::Update) {
        if let Some(changes) = &entry.changes {
            let diff = format_expression_changes(changes);
            if !diff.is_empty() {
                description.push_str(&format!("\n\n{diff}"));
            }
        }
    }

    if let Some(reason) = &entry.reason {
        description.push_str(&format!("\n\nReason:\n```{reason} ```"));
    }

    let mut embed = CreateEmbed::new().color(color).description(description);

    if !matches!(action, StickerAction::Update) {
        if let Some(id) = sticker_id {
            embed = embed.image(format!(
                "https://media.discordapp.net/stickers/{}.webp?size=128",
                id
            ));
        }
    }

    let msg = CreateMessage::new().add_embed(embed);
    guild_log(&ctx, LogType::Expressions, guild_id, msg, None).await;
}

fn format_expression_changes(changes: &[Change]) -> String {
    let mut lines = Vec::new();

    for change in changes {
        match change {
            Change::Name { old, new } => lines.push(field_diff("Name", old.clone(), new.clone())),
            Change::Description { old, new } => lines.push(field_diff("Description", old.clone(), new.clone())),
            Change::Tags { old, new } => lines.push(field_diff("Tags", old.clone(), new.clone())),
            _ => {}
        }
    }

    lines.join("\n")
}
