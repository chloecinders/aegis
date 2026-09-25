use serenity::all::{ButtonStyle, Permissions};

use crate::command::Boxed;
use crate::command::error::{Error, Result};
use crate::domain::Snowflake;
use crate::features::automod::rule::{Mode, Rule};
use crate::features::automod::{store, ui};
use crate::platform::discord::interact::{
    Click, Control, Custom, Reaction, Router, Strangers, stale,
};
use crate::platform::ui::embed::Embed;
use crate::platform::ui::reply::Button;
use crate::platform::ui::tone::Tone;

pub fn detail(owner: Snowflake, rule: &Rule, back: Option<usize>) -> Vec<Button> {
    let mut out: Vec<_> = [Mode::Disabled, Mode::Active]
        .iter()
        .filter_map(|mode| {
            let mut parts = vec![rule.id.as_str().to_string(), mode.as_str().to_string()];

            if let Some(page) = back {
                parts.push(page.to_string());
            }

            let id = Custom::new("rule-mode", owner, parts).render()?;

            let label = match mode {
                Mode::Disabled => "Disabled",
                Mode::Active => "Active",
            };
            let style = match mode {
                Mode::Disabled => ButtonStyle::Secondary,
                Mode::Active => ButtonStyle::Danger,
            };

            Some(Button::new(id, label, style).disabled(*mode == rule.mode))
        })
        .collect();

    out.extend(
        Custom::new("rule-delete", owner, [rule.id.as_str()])
            .render()
            .map(|id| vec![Button::new(id, "Delete", ButtonStyle::Danger)])
            .unwrap_or_default(),
    );

    if let Some(page) = back
        && let Some(id) =
            Custom::new("rule-page", owner, [String::from("back"), page.to_string()]).render()
    {
        out.push(Button::new(id, "Back", ButtonStyle::Secondary));
    }

    out
}

pub fn browse(owner: Snowflake, rules: &[Rule], at: usize) -> Vec<Button> {
    let mut out = nav(owner, at, rules.len().div_ceil(5).max(1));

    out.extend(
        ui::page_of(rules, at)
            .iter()
            .enumerate()
            .filter_map(|(index, rule)| {
                let id = Custom::new(
                    "rule-open",
                    owner,
                    [rule.id.as_str().to_string(), at.to_string()],
                )
                .render()?;

                Some(Button::new(
                    id,
                    (index + 1).to_string(),
                    ButtonStyle::Secondary,
                ))
            }),
    );

    out
}

pub fn nav(owner: Snowflake, at: usize, total: usize) -> Vec<Button> {
    if total <= 1 {
        return Vec::new();
    }

    let last = total.saturating_sub(1);
    let steps: [(&str, usize, String, bool); 5] = [
        ("first", 0, String::from("<<"), at == 0),
        ("prev", at.saturating_sub(1), String::from("<"), at == 0),
        ("at", at, format!("{}/{total}", at + 1), true),
        ("next", (at + 1).min(last), String::from(">"), at == last),
        ("last", last, String::from(">>"), at == last),
    ];

    steps
        .into_iter()
        .filter_map(|(name, target, label, off)| {
            let id =
                Custom::new("rule-page", owner, [name.to_string(), target.to_string()]).render()?;

            Some(Button::new(id, label, ButtonStyle::Secondary).disabled(off))
        })
        .collect()
}

fn turn(click: &Click) -> Boxed<'_, Result<Reaction>> {
    Box::pin(async move {
        let Some(guild) = click.guild() else {
            return Ok(Reaction::private(stale()));
        };

        let rules = store::all(&click.app.pool, guild).await?;
        let at = click
            .part(1)
            .and_then(|raw| raw.parse::<usize>().ok())
            .unwrap_or_default()
            .min(rules.len().div_ceil(5).max(1).saturating_sub(1));

        Ok(Reaction::replace(
            ui::listing(&rules, at),
            browse(click.owner(), &rules, at),
        ))
    })
}

fn open(click: &Click) -> Boxed<'_, Result<Reaction>> {
    Box::pin(async move {
        let Some(id) = click.part(0) else {
            return Ok(Reaction::private(stale()));
        };

        let Some(rule) = store::by_id(&click.app.pool, id).await? else {
            return Err(Error::empty().title("rule not found"));
        };

        let back = click.part(1).and_then(|raw| raw.parse::<usize>().ok());

        Ok(Reaction::replace(
            ui::show(&rule),
            detail(click.owner(), &rule, back),
        ))
    })
}

fn set_mode(click: &Click) -> Boxed<'_, Result<Reaction>> {
    Box::pin(async move {
        let (Some(id), Some(mode)) = (click.part(0), click.part(1).and_then(Mode::parse)) else {
            return Ok(Reaction::private(stale()));
        };

        let Some(rule) = store::by_id(&click.app.pool, id).await? else {
            return Err(Error::empty().title("rule not found"));
        };

        store::set_mode(&click.app.pool, &rule.id, mode).await?;
        click.app.rules.forget(rule.guild);

        let enabled = Rule { mode, ..rule };
        let back = click.part(2).and_then(|raw| raw.parse::<usize>().ok());

        Ok(Reaction::replace(
            ui::show(&enabled),
            detail(click.owner(), &enabled, back),
        ))
    })
}

fn delete_rule(click: &Click) -> Boxed<'_, Result<Reaction>> {
    Box::pin(async move {
        let Some(id) = click.part(0) else {
            return Ok(Reaction::private(stale()));
        };

        let Some(rule) = store::by_id(&click.app.pool, id).await? else {
            return Err(Error::empty().title("rule not found"));
        };

        store::delete(&click.app.pool, rule.guild, &rule.name).await?;
        click.app.rules.forget(rule.guild);

        Ok(Reaction::replace(
            Embed::new("RULE DELETED")
                .subtitle(format!("Name: {}", rule.name))
                .tone(Tone::Danger),
            Vec::new(),
        ))
    })
}

pub fn register(router: &mut Router) {
    router.add(Control {
        key: "rule-mode",
        user: Permissions::MANAGE_GUILD,
        one_of: Permissions::empty(),
        strangers: Strangers::Deny,
        handle: set_mode,
    });

    router.add(Control {
        key: "rule-delete",
        user: Permissions::MANAGE_GUILD,
        one_of: Permissions::empty(),
        strangers: Strangers::Deny,
        handle: delete_rule,
    });

    router.add(Control {
        key: "rule-page",
        user: Permissions::MANAGE_GUILD,
        one_of: Permissions::empty(),
        strangers: Strangers::Fork,
        handle: turn,
    });

    router.add(Control {
        key: "rule-open",
        user: Permissions::MANAGE_GUILD,
        one_of: Permissions::empty(),
        strangers: Strangers::Fork,
        handle: open,
    });
}
