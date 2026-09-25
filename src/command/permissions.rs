use serenity::all::Permissions;

use crate::command::Meta;
use crate::command::caller::Caller;
use crate::command::error::{Error, Result};
use crate::features::permissions::resolve::{self, Decision, Request};
use crate::platform::discord::permissions::Actor;

pub async fn check_invocation(caller: &Caller, meta: &Meta) -> Result<()> {
    if meta.developer && !caller.app.is_developer(caller.author_id().get()) {
        return Err(Error::empty().title("👽"));
    }

    let guild = caller.guild().await?;
    let overwrites = &caller.overwrites().await?.entries;

    let bot = caller.bot_member().await?;
    let bot_permissions = guild.in_channel(
        Actor {
            id: bot.user.id,
            roles: &bot.roles,
        },
        overwrites,
    );

    if !bot_permissions.contains(Permissions::ADMINISTRATOR) && !bot_permissions.contains(meta.bot)
    {
        return Err(Error::new(caller.input())
            .title("bot missing required permissions")
            .with_all(format!(
                "missing {}",
                (meta.bot - bot_permissions).to_string().to_lowercase()
            )));
    }

    require_member_permission(caller, meta).await
}

pub async fn check_member_access(caller: &Caller, meta: &Meta) -> Result<()> {
    if meta.developer && !caller.app.is_developer(caller.author_id().get()) {
        return Err(Error::empty().title("👽"));
    }

    require_member_permission(caller, meta).await
}

async fn require_member_permission(caller: &Caller, meta: &Meta) -> Result<()> {
    let allowed = match resolve_rules(caller, meta).await? {
        Decision::Allowed { .. } => true,
        Decision::Denied { .. } => false,
        Decision::Default => has_default_permissions(caller, meta).await?,
    };

    match allowed {
        true => Ok(()),
        false => Err(Error::new(caller.input())
            .title("missing required permissions")
            .with_all("missing permissions")),
    }
}

async fn has_default_permissions(caller: &Caller, meta: &Meta) -> Result<bool> {
    let guild = caller.guild().await?;
    let overwrites = caller.overwrites().await?;
    let actor = caller.actor().await?;
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

pub async fn resolve_rules(caller: &Caller, meta: &Meta) -> Result<Decision> {
    let guild = caller.guild_snowflake()?;
    let set = caller.app.permits.compiled(caller.pool(), guild).await?;

    if set.is_empty() {
        return Ok(Decision::Default);
    }

    let actor = caller.actor().await?;
    let snapshot = caller.guild().await?;
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
            channel: caller.channel_id().get(),
            command: meta.name,
            category: meta.category,
            is_developer: caller.app.is_developer(caller.author_id().get()),
            is_owner: snapshot.owner == actor.user.id,
        },
    ))
}
