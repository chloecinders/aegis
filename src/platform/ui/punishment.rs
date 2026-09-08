use crate::domain::punishment::Punishment;
use crate::platform::text::duration;
use crate::platform::ui::embed::{Embed, mention};
use crate::platform::ui::marks::Marks;
use crate::platform::ui::tone::Tone;

fn qualifiers(punishment: &Punishment) -> Vec<String> {
    let mut parts = Vec::new();

    if punishment.verb.has_duration() {
        parts.push(format!(
            "Duration: {}",
            duration::phrase(punishment.duration)
        ));
    }

    if punishment.clear_days != 0 {
        let unit = match punishment.clear_days {
            1 => "day",
            _ => "days",
        };

        parts.push(format!(
            "Cleared {} {unit} of messages",
            punishment.clear_days
        ));
    }

    parts
}

pub fn log_entry(punishment: &Punishment, actor: Option<&str>, target: Option<&str>) -> Embed {
    let mut embed = Embed::new(punishment.verb.headline())
        .subtitle(format!("Log ID: `{}`", punishment.id))
        .subtitle(format!("Actor: {}", mention(punishment.actor, actor)))
        .subtitle(format!("Target: {}", mention(punishment.target, target)));

    for qualifier in qualifiers(punishment) {
        embed = embed.subtitle(qualifier);
    }

    embed.quote(punishment.reason.as_str()).tone(Tone::Info)
}

pub fn reply(punishment: &Punishment, marks: Marks) -> Embed {
    let mut embed = Embed::new(format!(
        "{} {}",
        mention(punishment.target, None),
        punishment.verb.shout()
    ))
    .subtitle(format!("Log ID: `{}`", punishment.id));

    for qualifier in qualifiers(punishment) {
        embed = embed.subtitle(qualifier);
    }

    marks
        .apply(embed)
        .quote(punishment.reason.as_str())
        .tone(Tone::Info)
}

pub fn notice(punishment: &Punishment, guild_name: &str) -> Embed {
    let mut embed = Embed::new(punishment.verb.shout()).subtitle(format!("Server: {guild_name}"));

    if punishment.verb.has_duration() {
        embed = embed.subtitle(format!(
            "Duration: {}",
            duration::phrase(punishment.duration)
        ));
    }

    embed.quote(punishment.reason.as_str()).tone(Tone::Info)
}
