use serenity::all::{ButtonStyle, CacheHttp, EditMessage, Permissions};
use sqlx::PgPool;

use crate::command::Boxed;
use crate::command::error::{Ctx, Error, Result};
use crate::domain::Snowflake;
use crate::domain::action::Action;
use crate::domain::ids::ActionId;
use crate::features::guildlog;
use crate::features::records;
use crate::features::references::{self, store, ui};
use crate::platform::discord::interact::{
    Click, Control, Custom, Reaction, Router, Strangers, stale,
};
use crate::platform::ui::reply::{self, Button};

pub fn button(owner: Snowflake, action: &ActionId) -> Option<Button> {
    let id = Custom::new("log-ref", owner, [action.as_str().to_string()]).render()?;

    Some(Button::new(id, "View Reference", ButtonStyle::Secondary))
}

pub async fn attach(pool: &PgPool, http: impl CacheHttp, action: &Action) -> Result<()> {
    let located = guildlog::store::locate(pool, action.guild, &action.id).await?;

    let Some((channel, message)) = located else {
        return Ok(());
    };

    let captured = store::load(pool, action.guild, &action.id).await?;
    let controls = records::controls::attached(
        action.actor,
        &action.id,
        captured.as_ref(),
        action.note.as_ref(),
    );

    if controls.is_empty() {
        return Ok(());
    }

    channel
        .widen()
        .edit_message(
            http.http(),
            message,
            EditMessage::new().components(vec![reply::row(&controls)]),
        )
        .await
        .ctx("attach the reference control")?;

    Ok(())
}

fn view(click: &Click) -> Boxed<'_, Result<Reaction>> {
    Box::pin(async move {
        let (Some(guild), Some(id)) = (click.guild(), click.part(0)) else {
            return Ok(Reaction::private(stale()));
        };

        let action = ActionId::from(id.to_string());

        let Some(mut captured) = store::load(&click.app.pool, guild, &action).await? else {
            return Err(Error::empty().title("reference not found"));
        };

        references::confirm(&click.app.pool, &click.ctx, &action, &mut captured).await?;

        Ok(Reaction::private(ui::viewed(guild, &captured)))
    })
}

pub fn register(router: &mut Router) {
    router.add(Control {
        key: "log-ref",
        user: Permissions::empty(),
        one_of: Permissions::MODERATE_MEMBERS
            .union(Permissions::KICK_MEMBERS)
            .union(Permissions::BAN_MEMBERS)
            .union(Permissions::MANAGE_NICKNAMES),
        strangers: Strangers::Fork,
        handle: view,
    });
}
