use chrono::Utc;

use crate::domain::Snowflake;
use crate::domain::action::Action;
use crate::domain::ids::ActionId;
use crate::domain::reason::Note;
use crate::features::references::{Attached, Captured};
use crate::platform::text::{duration, truncate};
use crate::platform::ui::embed::{Embed, codeblock, mention};
use crate::platform::ui::marks::Marks;
use crate::platform::ui::tone::Tone;

fn entry(at: usize, action: &Action, attached: Attached) -> String {
    let mut written = format!(
        "**{}\\. {}**\n-# Log ID: `{}` | Mod: {} | At: {}",
        at,
        action.verb.as_str().to_uppercase(),
        action.id,
        mention(action.actor, None),
        (format!("<t:{0}:d> <t:{0}:T>", action.created_at.timestamp()))
    );

    if let Some(expiry) = action.expires_at {
        let tense = match expiry > Utc::now() {
            true => "Expires",
            false => "Expired",
        };

        written.push_str(&format!(
            " | {tense}: {}",
            (format!("<t:{0}:d> <t:{0}:T>", expiry.timestamp()))
        ));
    }

    let carried: Vec<&str> = [
        action.note.is_some().then_some("+ note"),
        attached.content.then_some("+ ref"),
        attached.image.then_some("+ image"),
    ]
    .into_iter()
    .flatten()
    .collect();

    if !carried.is_empty() {
        written.push_str(&format!(" | {}", carried.join(", ")));
    }

    written.push('\n');
    written.push_str(&codeblock(&truncate::clamp(action.reason.as_str(), 100)));

    written
}

pub fn history(
    target: Snowflake,
    actions: &[Action],
    attached: &[Attached],
    page: u32,
    pages: u32,
    total: i64,
) -> Embed {
    let body = match actions.is_empty() {
        true => String::from("nothing on record"),
        false => actions
            .iter()
            .enumerate()
            .map(|(index, action)| {
                entry(
                    index + 1,
                    action,
                    attached.get(index).copied().unwrap_or_default(),
                )
            })
            .collect::<Vec<String>>()
            .join("\n\n"),
    };

    Embed::new("MEMBER LOG")
        .subtitle(format!("Target: {}", mention(target, None)))
        .subtitle(format!("Total: {total}"))
        .subtitle(format!("Page: {page} of {pages}"))
        .body(body)
        .tone(Tone::Info)
}

pub fn record(action: &Action, reference: Option<&Captured>) -> Embed {
    let mut embed = Embed::new(action.verb.shout())
        .subtitle(format!("Log ID: `{}`", action.id))
        .subtitle(format!("Target: {}", mention(action.target, None)))
        .subtitle(format!("Mod: {}", mention(action.actor, None)))
        .subtitle(format!(
            "At: {}",
            (format!("<t:{0}:d> <t:{0}:T>", action.created_at.timestamp()))
        ));

    if action.verb.has_duration() {
        embed = embed.subtitle(format!("Duration: {}", duration::phrase(action.duration())));
    }

    if let Some(expiry) = action.expires_at {
        let tense = match expiry > Utc::now() {
            true => "Expires",
            false => "Expired",
        };

        embed = embed.subtitle(format!(
            "{tense}: {}",
            (format!("<t:{0}:d> <t:{0}:T>", expiry.timestamp()))
        ));
    }

    if action.clear_days != 0 {
        let unit = match action.clear_days {
            1 => "day",
            _ => "days",
        };

        embed = embed.subtitle(format!("Cleared {} {unit} of messages", action.clear_days));
    }

    let marks = Marks {
        has_reference: reference.is_some_and(Captured::has_content),
        has_image: reference.is_some_and(Captured::has_image),
        ..Marks::default()
    };

    marks
        .apply(embed)
        .quote(action.reason.as_str())
        .tone(Tone::Info)
}

pub fn note(action: &ActionId, note: &Note) -> Embed {
    Embed::new("NOTE")
        .subtitle(format!("Log ID: `{action}`"))
        .quote(note.as_str())
        .tone(Tone::Info)
}

pub fn amended(action: &Action, field: &str, after: &str) -> Embed {
    Embed::new(format!("{} UPDATED", field.to_uppercase()))
        .subtitle(format!("Log ID: `{}`", action.id))
        .subtitle(format!("Target: {}", mention(action.target, None)))
        .quote(after)
        .tone(Tone::Info)
}

pub fn deleted(action: &Action) -> Embed {
    Embed::new("LOG DELETED")
        .subtitle(format!("Log ID: `{}`", action.id))
        .subtitle(format!("Target: {}", mention(action.target, None)))
        .tone(Tone::Danger)
}

pub fn cleared(action: &Action, field: &str) -> Embed {
    Embed::new(format!("{} CLEARED", field.to_uppercase()))
        .subtitle(format!("Log ID: `{}`", action.id))
        .subtitle(format!("Target: {}", mention(action.target, None)))
        .tone(Tone::Info)
}
