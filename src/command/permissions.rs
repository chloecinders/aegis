use serenity::all::Permissions;

use crate::command::Meta;
use crate::command::cx::Cx;
use crate::command::error::{Error, Result};
use crate::features::permissions::resolve::{self, Decision, Request};
use crate::platform::discord::permissions::Actor;

pub async fn statics(cx: &Cx, meta: &Meta) -> Result<()> {
    if meta.developer && !cx.app.is_developer(cx.author_id().get()) {
        return Err(Error::bare().title("👽"));
    }

    let guild = cx.guild().await?;
    let overwrites = &cx.overwrites().await?.entries;

    let bot = cx.bot_member().await?;
    let bot_permissions = guild.in_channel(
        Actor {
            id: bot.user.id,
            roles: &bot.roles,
        },
        overwrites,
    );

    if !bot_permissions.contains(Permissions::ADMINISTRATOR) && !bot_permissions.contains(meta.bot)
    {
        return Err(Error::new(cx.input())
            .title("bot missing required permissions")
            .with_all(format!(
                "missing {}",
                (meta.bot - bot_permissions).to_string().to_lowercase()
            )));
    }

    entitled(cx, meta).await
}

pub async fn may(cx: &Cx, meta: &Meta) -> Result<()> {
    if meta.developer && !cx.app.is_developer(cx.author_id().get()) {
        return Err(Error::bare().title("👽"));
    }

    entitled(cx, meta).await
}

async fn entitled(cx: &Cx, meta: &Meta) -> Result<()> {
    let allowed = match granular(cx, meta).await? {
        Decision::Allowed { .. } => true,
        Decision::Denied { .. } => false,
        Decision::Default => wields(cx, meta).await?,
    };

    match allowed {
        true => Ok(()),
        false => Err(Error::new(cx.input())
            .title("missing required permissions")
            .with_all("missing permissions")),
    }
}

async fn wields(cx: &Cx, meta: &Meta) -> Result<bool> {
    let guild = cx.guild().await?;
    let overwrites = cx.overwrites().await?;
    let actor = cx.actor().await?;
    let permissions = guild.in_channel(
        Actor {
            id: actor.user.id,
            roles: &actor.roles,
        },
        &overwrites.entries,
    );

    if permissions.contains(Permissions::ADMINISTRATOR) {
        return Ok(true);
    }

    let one_of = meta.one_of.is_empty() || !permissions.intersection(meta.one_of).is_empty();

    Ok(permissions.contains(meta.user) && one_of)
}

pub async fn granular(cx: &Cx, meta: &Meta) -> Result<Decision> {
    let guild = cx.guild_snowflake()?;
    let set = cx.app.permits.compiled(cx.pool(), guild).await?;

    if set.is_empty() {
        return Ok(Decision::Default);
    }

    let actor = cx.actor().await?;
    let snapshot = cx.guild().await?;
    let roles: Vec<(u64, i64)> = actor
        .roles
        .iter()
        .map(|id| {
            let position = snapshot.roles.get(id).map(|role| role.position);

            (id.get(), position.unwrap_or_default())
        })
        .collect();

    Ok(resolve::resolve(
        &set,
        &Request {
            member: actor.user.id.get(),
            roles: &roles,
            channel: cx.channel_id().get(),
            command: meta.name,
            category: meta.category,
            is_developer: cx.app.is_developer(cx.author_id().get()),
            is_owner: snapshot.owner == actor.user.id,
        },
    ))
}
