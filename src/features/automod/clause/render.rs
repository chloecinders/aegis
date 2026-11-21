use crate::features::automod::rule::{Body, Condition, Notify, Source};
use crate::platform::text::duration;

use super::token::Mention;

pub fn render(body: &Body) -> String {
    let mut out = Vec::new();

    if !body.sources.is_empty() {
        out.push(format!(
            "on {}",
            body.sources
                .iter()
                .map(Source::as_str)
                .collect::<Vec<&str>>()
                .join(" ")
        ));
    }

    out.extend(
        body.matches
            .iter()
            .map(|matcher| format!("match {}", matcher.render())),
    );

    out.extend(
        body.nevers
            .iter()
            .map(|matcher| format!("never {}", matcher.render())),
    );

    out.extend(body.conditions.iter().map(Condition::render));

    if !body.ignore_roles.is_empty()
        || !body.ignore_users.is_empty()
        || !body.ignore_channels.is_empty()
        || !body.ignore_permissions.is_empty()
    {
        let roles = body.ignore_roles.iter().map(|id| Mention::Role.tag(*id));
        let users = body.ignore_users.iter().map(|id| format!("user:{id}"));
        let channels = body
            .ignore_channels
            .iter()
            .map(|id| Mention::Channel.tag(*id));
        let permissions = body
            .ignore_permissions
            .iter_names()
            .map(|(name, _)| format!("permission:{}", name.to_lowercase()))
            .collect::<Vec<String>>();

        out.push(format!(
            "ignore {}",
            roles
                .chain(users)
                .chain(channels)
                .chain(permissions)
                .collect::<Vec<String>>()
                .join(" ")
        ));
    }

    if !body.only.is_empty() {
        out.push(format!(
            "only {}",
            body.only
                .iter()
                .map(|id| Mention::Channel.tag(*id))
                .collect::<Vec<String>>()
                .join(" ")
        ));
    }

    if let Some(threshold) = body.after {
        out.push(format!(
            "after {} in {}",
            threshold.count,
            duration::compact(threshold.window)
        ));
    }

    let outcome = &body.outcome;

    match outcome.punishment_type {
        Some(verb) if verb.has_duration() && !outcome.duration.is_zero() => out.push(format!(
            "then {} {}",
            verb.as_str(),
            duration::compact(outcome.duration)
        )),
        Some(verb) => out.push(format!("then {}", verb.as_str())),
        None if outcome.delete => out.push(String::from("then delete")),
        None => {}
    }

    if outcome.delete && outcome.punishment_type.is_some() {
        out.push(String::from("delete"));
    }

    if outcome.clear_days > 0 {
        out.push(format!("clear {}", outcome.clear_days));
    }

    match outcome.notify {
        Notify::Log => {}
        Notify::None => out.push(String::from("notify none")),
        Notify::Channel(channel) => {
            out.push(format!("notify {}", Mention::Channel.tag(channel)));
        }
    }

    if let Some(reason) = &outcome.reason {
        out.push(format!("reason {reason}"));
    }

    out.join("\n")
}
