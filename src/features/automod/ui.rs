use crate::features::automod::clause;
use crate::features::automod::eval::Hit;
use crate::features::automod::rule::{Mode, Notify, Outcome, Rule};
use crate::platform::text::truncate;
use crate::platform::ui::embed::{self, Embed};
use crate::platform::ui::tone::Tone;

pub fn tone(mode: Mode) -> Tone {
    match mode {
        Mode::Active => Tone::Success,
        Mode::Disabled => Tone::Info,
    }
}

pub fn show(rule: &Rule) -> Embed {
    Embed::new(format!("RULE {}", rule.name.to_uppercase()))
        .subtitle(format!("ID: `{}`", rule.id))
        .subtitle(format!("Mode: {}", rule.mode))
        .quote(clause::render(&rule.body))
        .tone(tone(rule.mode))
}

pub fn saved(rule: &Rule, replaced: bool) -> Embed {
    let headline = match replaced {
        true => "RULE UPDATED",
        false => "RULE CREATED",
    };

    Embed::new(headline)
        .subtitle(format!("ID: `{}`", rule.id))
        .subtitle(format!("Name: {}", rule.name))
        .quote(clause::render(&rule.body))
        .tone(tone(rule.mode))
}

pub fn page_of(rules: &[Rule], at: usize) -> &[Rule] {
    let start = (at * 5).min(rules.len());
    let end = (start + 5).min(rules.len());

    &rules[start..end]
}

fn entry(rule: &Rule) -> String {
    format!(
        "**{}**\n-# Rule ID: `{}` | Mode: {}",
        truncate::clamp(&rule.name, 22).to_uppercase(),
        rule.id,
        rule.mode.as_str()
    )
}

pub fn listing(rules: &[Rule], at: usize) -> Embed {
    if rules.is_empty() {
        return Embed::new("NO RULES FOUND")
            .body("Rules will show up here, run `rule help` for more information.")
            .tone(Tone::Danger);
    }

    let total = rules.len().div_ceil(5).max(1);

    Embed::new("AUTOMOD RULES")
        .body(
            page_of(rules, at)
                .iter()
                .map(entry)
                .collect::<Vec<String>>()
                .join("\n\n"),
        )
        .footnote(format!(
            "Pick a rule below, or run `rule show <name>` | page {} of {total}",
            at + 1
        ))
        .tone(Tone::Info)
}

pub fn action(outcome: &Outcome) -> String {
    let mut parts: Vec<String> = Vec::new();

    if let Some(verb) = outcome.punishment_type {
        parts.push(String::from(verb.as_str()));
    }

    if outcome.delete {
        parts.push(String::from("delete"));
    }

    if matches!(outcome.notify, Notify::Channel(_)) {
        parts.push(String::from("notify"));
    }

    match parts.is_empty() {
        true => String::from("nothing"),
        false => parts.join(" + "),
    }
}

pub fn triggered(hit: &Hit, target: u64, name: Option<&str>) -> Embed {
    Embed::new("AUTOMOD TRIGGERED")
        .subtitle(format!("Rule ID: `{}`", hit.rule))
        .subtitle(format!("Target: {}", embed::mention(target, name)))
        .tone(Tone::Danger)
}
