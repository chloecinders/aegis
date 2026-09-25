use serenity::all::{ButtonStyle, Permissions};

use crate::command::Boxed;
use crate::command::error::{Error, Result};
use crate::domain::Snowflake;
use crate::features::archive::commands::snipe;
use crate::platform::discord::interact::{Click, Control, Custom, Reaction, Router, Strangers};
use crate::platform::ui::reply::Button;

pub fn nav(owner: Snowflake, at: u64, total: u64) -> Vec<Button> {
    if total <= 1 {
        return Vec::new();
    }

    let last = total.saturating_sub(1);
    let steps: [(&str, u64, String, bool); 5] = [
        ("first", 0, String::from("<<"), at == 0),
        ("prev", at.saturating_sub(1), String::from("<"), at == 0),
        ("at", at, format!("{}/{total}", at + 1), true),
        ("next", (at + 1).min(last), String::from(">"), at == last),
        ("last", last, String::from(">>"), at == last),
    ];

    steps
        .into_iter()
        .filter_map(|(name, target, label, off)| {
            let id = Custom::new("snipe-page", owner, [name.to_string(), target.to_string()])
                .render()?;

            Some(Button::new(id, label, ButtonStyle::Secondary).disabled(off))
        })
        .collect()
}

fn turn(click: &Click) -> Boxed<'_, Result<Reaction>> {
    Box::pin(async move {
        let Some(guild) = click.guild() else {
            return Err(Error::empty().title("command only works in servers"));
        };

        let at = click
            .part(1)
            .and_then(|raw| raw.parse::<u64>().ok())
            .unwrap_or_default();

        let (embed, buttons) = snipe::page(
            &click.app,
            &click.ctx,
            guild,
            click.interaction.channel.get(),
            click.owner(),
            at,
        )
        .await?;

        Ok(Reaction::replace(embed, buttons))
    })
}

pub fn register(router: &mut Router) {
    router.add(Control {
        key: "snipe-page",
        user: Permissions::empty(),
        one_of: Permissions::empty(),
        strangers: Strangers::Deny,
        handle: turn,
    });
}
