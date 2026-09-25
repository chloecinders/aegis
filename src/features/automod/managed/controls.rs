use serenity::all::{ButtonStyle, Permissions};

use crate::command::Boxed;
use crate::command::error::{Error, Result};
use crate::domain::Snowflake;
use crate::features::automod::managed::{self, Offer};
use crate::features::automod::rule::Mode;
use crate::platform::discord::interact::{
    Click, Control, Custom, Reaction, Router, Strangers, stale,
};
use crate::platform::ui::embed::Embed;
use crate::platform::ui::reply::Button;
use crate::platform::ui::tone::Tone;

pub fn status(owner: Snowflake, offer: &Offer, back: Option<usize>) -> Vec<Button> {
    let Some(subscription) = &offer.subscription else {
        return Vec::new();
    };

    [Mode::Disabled, Mode::Active]
        .iter()
        .filter_map(|mode| {
            let mut parts = vec![
                offer.managed.id.as_str().to_string(),
                mode.as_str().to_string(),
            ];

            if let Some(page) = back {
                parts.push(page.to_string());
            }

            let id = Custom::new("managed-mode", owner, parts).render()?;
            let (label, style) = match mode {
                Mode::Disabled => ("Disabled", ButtonStyle::Secondary),
                Mode::Active => ("Active", ButtonStyle::Danger),
            };

            Some(Button::new(id, label, style).disabled(*mode == subscription.mode))
        })
        .collect()
}

pub fn membership(owner: Snowflake, offer: &Offer, back: Option<usize>) -> Vec<Button> {
    let joining = offer.subscription.is_none();

    if joining && offer.managed.mode != Mode::Active {
        return Vec::new();
    }

    let mut parts = vec![
        offer.managed.id.as_str().to_string(),
        match joining {
            true => String::from("add"),
            false => String::from("remove"),
        },
    ];

    if let Some(page) = back {
        parts.push(page.to_string());
    }

    let (name, tone) = match joining {
        true => ("Subscribe", ButtonStyle::Success),
        false => ("Unsubscribe", ButtonStyle::Danger),
    };

    Custom::new("managed-join", owner, parts)
        .render()
        .map(|id| vec![Button::new(id, name, tone)])
        .unwrap_or_default()
}

pub fn all(owner: Snowflake, offer: &Offer, back: Option<usize>) -> Vec<Button> {
    let mut out = status(owner, offer, back);

    out.extend(membership(owner, offer, back));

    if let Some(page) = back
        && let Some(id) = Custom::new(
            "managed-page",
            owner,
            [String::from("back"), page.to_string()],
        )
        .render()
    {
        out.push(Button::new(id, "Back", ButtonStyle::Secondary));
    }

    out
}

pub fn browse(owner: Snowflake, offers: &[Offer], at: usize) -> Vec<Button> {
    let mut out = nav(owner, at, offers.len().div_ceil(5).max(1));

    out.extend(
        managed::ui::page_of(offers, at)
            .iter()
            .enumerate()
            .filter_map(|(index, offer)| {
                let id = Custom::new(
                    "managed-open",
                    owner,
                    [offer.managed.id.as_str().to_string(), at.to_string()],
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
            let id = Custom::new(
                "managed-page",
                owner,
                [name.to_string(), target.to_string()],
            )
            .render()?;

            Some(Button::new(id, label, ButtonStyle::Secondary).disabled(off))
        })
        .collect()
}

async fn load_offer(click: &Click, guild: u64, id: &str) -> Result<Offer> {
    let managed = managed::store::by_id(&click.app.pool, id)
        .await?
        .ok_or_else(|| Error::empty().title("managed rule not found"))?;
    let subscription = managed::store::subscription(&click.app.pool, guild, &managed.id).await?;

    Ok(Offer {
        managed,
        subscription,
    })
}

fn turn(click: &Click) -> Boxed<'_, Result<Reaction>> {
    Box::pin(async move {
        let Some(guild) = click.guild() else {
            return Ok(Reaction::private(stale()));
        };

        let offers = managed::store::offers(&click.app.pool, guild).await?;
        let at = click
            .part(1)
            .and_then(|raw| raw.parse::<usize>().ok())
            .unwrap_or_default()
            .min(offers.len().div_ceil(5).max(1).saturating_sub(1));

        Ok(Reaction::replace(
            managed::ui::listing(&offers, at),
            browse(click.owner(), &offers, at),
        ))
    })
}

fn open(click: &Click) -> Boxed<'_, Result<Reaction>> {
    Box::pin(async move {
        let (Some(guild), Some(id)) = (click.guild(), click.part(0)) else {
            return Ok(Reaction::private(stale()));
        };

        let offer = load_offer(click, guild, id).await?;
        let back = click.part(1).and_then(|raw| raw.parse::<usize>().ok());

        Ok(Reaction::replace(
            managed::ui::offer(&offer),
            all(click.owner(), &offer, back),
        ))
    })
}

fn set_mode(click: &Click) -> Boxed<'_, Result<Reaction>> {
    Box::pin(async move {
        let (Some(guild), Some(id), Some(mode)) = (
            click.guild(),
            click.part(0),
            click.part(1).and_then(Mode::parse),
        ) else {
            return Ok(Reaction::private(stale()));
        };

        let mut offer = load_offer(click, guild, id).await?;

        if offer.subscription.is_none() {
            return Err(Error::empty().title("server not subscribed to rule"));
        }

        managed::store::set_guild_mode(&click.app.pool, guild, &offer.managed.id, mode).await?;
        click.app.rules.forget(guild);

        offer.subscription =
            managed::store::subscription(&click.app.pool, guild, &offer.managed.id).await?;

        let back = click.part(2).and_then(|raw| raw.parse::<usize>().ok());

        Ok(Reaction::replace(
            managed::ui::offer(&offer),
            all(click.owner(), &offer, back),
        ))
    })
}

fn join(click: &Click) -> Boxed<'_, Result<Reaction>> {
    Box::pin(async move {
        let (Some(guild), Some(id), Some(verb)) = (click.guild(), click.part(0), click.part(1))
        else {
            return Ok(Reaction::private(stale()));
        };

        let mut offer = load_offer(click, guild, id).await?;
        let back = click.part(2).and_then(|raw| raw.parse::<usize>().ok());

        if verb == "remove" {
            managed::store::unsubscribe(&click.app.pool, guild, &offer.managed.id).await?;
            click.app.rules.forget(guild);

            return Ok(Reaction::replace(
                Embed::new("MANAGED RULE UNSUBSCRIBED")
                    .subtitle(format!("Name: {}", offer.managed.name))
                    .tone(Tone::Danger),
                Vec::new(),
            ));
        }

        if offer.managed.mode != Mode::Active {
            return Ok(Reaction::private(stale()));
        }

        managed::store::subscribe(&click.app.pool, guild, &offer.managed.id).await?;
        click.app.rules.forget(guild);

        offer.subscription =
            managed::store::subscription(&click.app.pool, guild, &offer.managed.id).await?;

        Ok(Reaction::replace(
            managed::ui::offer(&offer),
            all(click.owner(), &offer, back),
        ))
    })
}

pub fn register(router: &mut Router) {
    router.add(Control {
        key: "managed-mode",
        user: Permissions::MANAGE_GUILD,
        one_of: Permissions::empty(),
        strangers: Strangers::Deny,
        handle: set_mode,
    });

    router.add(Control {
        key: "managed-join",
        user: Permissions::MANAGE_GUILD,
        one_of: Permissions::empty(),
        strangers: Strangers::Deny,
        handle: join,
    });

    router.add(Control {
        key: "managed-page",
        user: Permissions::MANAGE_GUILD,
        one_of: Permissions::empty(),
        strangers: Strangers::Fork,
        handle: turn,
    });

    router.add(Control {
        key: "managed-open",
        user: Permissions::MANAGE_GUILD,
        one_of: Permissions::empty(),
        strangers: Strangers::Fork,
        handle: open,
    });
}
