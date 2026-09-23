use serenity::all::audit_log::{Action, Change, EmojiAction, VoiceChannelStatusAction};
use serenity::all::{ChannelAction, Permissions, RoleAction, StickerAction};

use crate::domain::Snowflake;
use crate::domain::logtype::LogType;
use crate::features::guildlog::attribution::Attribution;
use crate::platform::ui::embed::{Embed, channel_mention, code, role_mention};
use crate::platform::ui::tone::Tone;

pub struct Event<'a> {
    pub action: Action,
    pub target: Option<Snowflake>,
    pub actor: Attribution,
    pub actor_name: Option<&'a str>,
    pub bot: Snowflake,
    pub changes: &'a [Change],
    pub status: Option<&'a str>,
    pub reason: Option<&'a str>,
}

pub struct Logged {
    pub kind: LogType,
    pub embed: Embed,
}

pub fn render(seen: &Event) -> Option<Logged> {
    match seen.action {
        Action::Channel(what) => channel(seen, what),
        Action::Role(what) => role(seen, what),
        Action::Emoji(what) => emoji(seen, what),
        Action::Sticker(what) => sticker(seen, what),
        Action::VoiceChannelStatus(what) => voice(seen, what),
        _ => None,
    }
}

fn channel(seen: &Event, what: ChannelAction) -> Option<Logged> {
    let (title, tone, shape) = match what {
        ChannelAction::Create => ("CHANNEL CREATED", Tone::Success, Shape::Created),
        ChannelAction::Update => ("CHANNEL UPDATED", Tone::Warn, Shape::Updated),
        _ => return None,
    };

    let embed = Embed::new(title)
        .subtitle(match seen.target {
            Some(id) => format!("Channel: {}", channel_mention(id)),
            None => String::from("Channel: unknown"),
        })
        .maybe_subtitle(seen.actor.line(seen.bot, seen.actor_name))
        .lead(paragraph(channel_changes(shape, seen.changes)))
        .maybe_quote(reason(seen))
        .tone(tone);

    Some(Logged {
        kind: LogType::Channels,
        embed,
    })
}

fn role(seen: &Event, what: RoleAction) -> Option<Logged> {
    let (title, tone, shape) = match what {
        RoleAction::Create => ("ROLE CREATED", Tone::Success, Shape::Created),
        RoleAction::Update => ("ROLE UPDATED", Tone::Warn, Shape::Updated),
        RoleAction::Delete => ("ROLE DELETED", Tone::Danger, Shape::Deleted),
        _ => return None,
    };

    let embed = Embed::new(title)
        .subtitle(match (name_of(seen.changes), seen.target) {
            (Some(name), _) => format!("Role: {}", code(&format!("@{name}"))),
            (None, Some(id)) => format!("Role: {}", role_mention(id)),
            (None, None) => String::from("Role: unknown"),
        })
        .maybe_subtitle(seen.actor.line(seen.bot, seen.actor_name))
        .lead(paragraph(role_changes(shape, seen.changes)))
        .maybe_quote(reason(seen))
        .tone(tone);

    Some(Logged {
        kind: LogType::Roles,
        embed,
    })
}

fn emoji(seen: &Event, what: EmojiAction) -> Option<Logged> {
    let (title, tone, shape) = match what {
        EmojiAction::Create => ("EMOJI CREATED", Tone::Success, Shape::Created),
        EmojiAction::Update => ("EMOJI UPDATED", Tone::Warn, Shape::Updated),
        EmojiAction::Delete => ("EMOJI DELETED", Tone::Danger, Shape::Deleted),
        _ => return None,
    };

    let name = name_of(seen.changes);
    let inline = match (&name, seen.target) {
        (Some(name), Some(id)) => Some(format!("Emoji: <:{name}:{id}>")),
        (Some(name), None) => Some(format!("Emoji: {}", code(&format!(":{name}:")))),
        (None, _) => None,
    };

    let picture = match what {
        EmojiAction::Update => None,
        _ => seen
            .target
            .map(|id| format!("https://cdn.discordapp.com/emojis/{id}.webp?size=128")),
    };

    let embed = Embed::new(title)
        .maybe_subtitle(inline)
        .maybe_subtitle(seen.target.map(|id| format!("ID: `{id}`")))
        .maybe_subtitle(seen.actor.line(seen.bot, seen.actor_name))
        .lead(paragraph(expression_changes(shape, seen.changes)))
        .maybe_quote(reason(seen))
        .maybe_image(picture)
        .tone(tone);

    Some(Logged {
        kind: LogType::Expressions,
        embed,
    })
}

fn sticker(seen: &Event, what: StickerAction) -> Option<Logged> {
    let (title, tone, shape) = match what {
        StickerAction::Create => ("STICKER CREATED", Tone::Success, Shape::Created),
        StickerAction::Update => ("STICKER UPDATED", Tone::Warn, Shape::Updated),
        StickerAction::Delete => ("STICKER DELETED", Tone::Danger, Shape::Deleted),
        _ => return None,
    };

    let picture = match what {
        StickerAction::Update => None,
        _ => seen
            .target
            .map(|id| format!("https://media.discordapp.net/stickers/{id}.webp?size=128")),
    };

    let embed = Embed::new(title)
        .subtitle(match name_of(seen.changes) {
            Some(name) => format!("Sticker: {}", code(&name)),
            None => String::from("Sticker: unknown"),
        })
        .maybe_subtitle(seen.target.map(|id| format!("ID: `{id}`")))
        .maybe_subtitle(seen.actor.line(seen.bot, seen.actor_name))
        .lead(paragraph(expression_changes(shape, seen.changes)))
        .maybe_quote(reason(seen))
        .maybe_image(picture)
        .tone(tone);

    Some(Logged {
        kind: LogType::Expressions,
        embed,
    })
}

fn voice(seen: &Event, what: VoiceChannelStatusAction) -> Option<Logged> {
    let title = match what {
        VoiceChannelStatusAction::StatusCreate => "VOICE STATUS SET",
        VoiceChannelStatusAction::StatusDelete => "VOICE STATUS CLEARED",
        _ => return None,
    };

    let stated = Embed::new(title)
        .subtitle(match seen.target {
            Some(id) => format!("Channel: {}", channel_mention(id)),
            None => String::from("Channel: unknown"),
        })
        .maybe_subtitle(seen.actor.line(seen.bot, seen.actor_name));

    let embed = match what {
        VoiceChannelStatusAction::StatusCreate => stated.lead(format!(
            "Status: {}",
            match seen.status {
                Some(status) => code(status),
                None => String::from("(none)"),
            }
        )),
        _ => stated.lead("the status was cleared"),
    }
    .maybe_quote(reason(seen))
    .tone(Tone::Warn);

    Some(Logged {
        kind: LogType::VoiceActivity,
        embed,
    })
}

fn name_of(changes: &[Change]) -> Option<String> {
    changes.iter().find_map(|change| match change {
        Change::Name { old, new } => new.as_ref().or(old.as_ref()).map(ToString::to_string),
        _ => None,
    })
}

fn reason(seen: &Event) -> Option<String> {
    seen.reason
        .map(str::trim)
        .filter(|given| !given.is_empty())
        .map(str::to_owned)
}

fn paragraph(lines: Vec<String>) -> String {
    match lines.is_empty() {
        true => String::from("no recorded changes"),
        false => lines.join("\n"),
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Shape {
    Created,
    Updated,
    Deleted,
}

fn spelled<T: ToString>(value: &Option<T>) -> Option<String> {
    value.as_ref().map(ToString::to_string)
}

fn field(shape: Shape, label: &str, old: Option<String>, new: Option<String>) -> Option<String> {
    let old = old.filter(|value| !value.is_empty());
    let new = new.filter(|value| !value.is_empty());
    let stated = |value: String| format!("{label}: {}", code(&value));

    match shape {
        Shape::Created => new.or(old).map(stated),
        Shape::Deleted => old.or(new).map(stated),
        Shape::Updated => match (old, new) {
            (Some(old), Some(new)) if old != new => {
                Some(format!("{label}: {} -> {}", code(&old), code(&new)))
            }
            (None, Some(new)) => Some(format!("{label}: (none) -> {}", code(&new))),
            (Some(old), None) => Some(format!("{label}: {} -> (none)", code(&old))),
            _ => None,
        },
    }
}

fn channel_changes(shape: Shape, changes: &[Change]) -> Vec<String> {
    changes
        .iter()
        .filter_map(|change| match change {
            Change::Name { old, new } => field(shape, "Name", spelled(old), spelled(new)),
            Change::Topic { old, new } => field(shape, "Topic", spelled(old), spelled(new)),
            Change::Nsfw { old, new } => field(
                shape,
                "NSFW",
                old.map(|set| String::from(if set { "yes" } else { "no" })),
                new.map(|set| String::from(if set { "yes" } else { "no" })),
            ),
            Change::Bitrate { old, new } => field(
                shape,
                "Bitrate",
                old.map(|rate| format!("{rate}bps")),
                new.map(|rate| format!("{rate}bps")),
            ),
            Change::RateLimitPerUser { old, new } => field(
                shape,
                "Slowmode",
                old.map(|secs| format!("{secs}s")),
                new.map(|secs| format!("{secs}s")),
            ),
            Change::UserLimit { old, new } => field(
                shape,
                "User limit",
                old.map(|cap| cap.to_string()),
                new.map(|cap| cap.to_string()),
            ),
            Change::Position { old, new } => field(
                shape,
                "Position",
                old.map(|at| at.to_string()),
                new.map(|at| at.to_string()),
            ),
            _ => None,
        })
        .collect()
}

fn role_changes(shape: Shape, changes: &[Change]) -> Vec<String> {
    changes
        .iter()
        .filter_map(|change| match change {
            Change::Name { old, new } => field(shape, "Name", spelled(old), spelled(new)),
            Change::Color { old, new } => field(
                shape,
                "Color",
                old.map(|rgb| format!("#{rgb:06X}")),
                new.map(|rgb| format!("#{rgb:06X}")),
            ),
            Change::Hoist { old, new } => field(
                shape,
                "Hoisted",
                old.map(|set| String::from(if set { "yes" } else { "no" })),
                new.map(|set| String::from(if set { "yes" } else { "no" })),
            ),
            Change::Mentionable { old, new } => field(
                shape,
                "Mentionable",
                old.map(|set| String::from(if set { "yes" } else { "no" })),
                new.map(|set| String::from(if set { "yes" } else { "no" })),
            ),
            Change::Permissions { old, new } => permissions(shape, *old, *new),
            _ => None,
        })
        .collect()
}

fn permissions(shape: Shape, old: Option<Permissions>, new: Option<Permissions>) -> Option<String> {
    let listed = |what: Option<Permissions>| match what {
        Some(permissions) if !permissions.is_empty() => {
            Some(format!("Permissions: {}", permission_names(permissions)))
        }
        _ => None,
    };

    match shape {
        Shape::Created => return listed(new.or(old)),
        Shape::Deleted => return listed(old.or(new)),
        Shape::Updated => (),
    }

    let (old, new) = (old?, new?);

    if old == new {
        return None;
    }

    let lines: Vec<String> = [("granted", new & !old), ("revoked", old & !new)]
        .into_iter()
        .filter(|(_, moved)| !moved.is_empty())
        .map(|(label, moved)| format!("Permissions {label}: {}", permission_names(moved)))
        .collect();

    match lines.is_empty() {
        true => None,
        false => Some(lines.join("\n")),
    }
}

fn permission_names(permissions: Permissions) -> String {
    permissions
        .get_permission_names()
        .into_iter()
        .map(code)
        .collect::<Vec<_>>()
        .join(", ")
}

fn expression_changes(shape: Shape, changes: &[Change]) -> Vec<String> {
    changes
        .iter()
        .filter_map(|change| match change {
            Change::Name { old, new } => field(shape, "Name", spelled(old), spelled(new)),
            Change::Description { old, new } => {
                field(shape, "Description", spelled(old), spelled(new))
            }
            Change::Tags { old, new } => field(shape, "Tags", spelled(old), spelled(new)),
            Change::Available { old, new } => field(
                shape,
                "Available",
                old.map(|set| String::from(if set { "yes" } else { "no" })),
                new.map(|set| String::from(if set { "yes" } else { "no" })),
            ),
            _ => None,
        })
        .collect()
}
