use serenity::all::{ChannelId, RoleId};

use crate::command::cx::Cx;
use crate::command::error::{Error, Result};
use crate::command::{Command, Meta, Response};
use crate::domain::Snowflake;
use crate::features::permissions::rule::{self, Effect, Rule, Scope, Target};
use crate::features::permissions::{store, ui};
use crate::platform::ui::embed::Embed;
use crate::platform::ui::tone::Tone;
use aegis_macros::{command, meta};

#[command]
pub struct Perms {
    #[arg]
    action: Option<String>,
    #[arg]
    subject: Option<String>,
    #[arg]
    target: Option<String>,
    #[flag(
        short = 'p',
        desc = "Where this rule sits when more than one answers the same command"
    )]
    priority: Option<i32>,
}

impl Command for Perms {
    const META: Meta = meta! {
        name: "perms",
        aliases: ["perm", "permissions"],
        short: "Granular permission management",
        full: "Allows for granular permission management. By default every command checks the users permission against \
        the commands required Discord permissions. So the ban command requires for example `BAN_MEMBERS`. Any user with \
        the ban members permission can use the ban command. Some server setups want specific users or roles to have access to \
        specific commands, without granting the Discord permission. This command is used to manage granular permissions, \
        allowing you to define if specific roles can or can't use commands despite their Discord permissions.\n\n\
        `/p/perms allow <subject> <command>` grants the subject permissions for that command. `deny` instead of `allow` takes \
        the permission away. Subject can be either a user, role or a channel, written as a plain id, as `role:<id>`, \
        `channel:<id>` or `member:<id>`, or as a mention. Instead of `<command>` you can also use \
        command categories or `*` for all commands (Also bot administrative commands!). `/p/perms list` shows you all \
        custom permission rules. `/p/perms remove <id>` removes a permission rule while `/p/perms clear` removes all permission rules.\n\n\
        Every rule has a priority of 0 by default. If two rules would match on the same command, the rule with the highest priority wins. \
        I.e. `/p/perms deny #general *` (deny all commands in general) then `/p/perms allow @mods moderation +p 1` would still \
        allow moderators to use permission commands in general.",
        category: Admin,
        user: [MANAGE_GUILD],
        edit: Rerun,
    };

    async fn run(self, cx: &mut Cx) -> Result<Response> {
        let guild = cx.guild_snowflake()?;

        let Some(action) = self.action.as_deref() else {
            return list(cx, guild).await;
        };

        match action.to_lowercase().as_str() {
            "list" => list(cx, guild).await,
            "clear" => clear(cx, guild).await,
            "remove" | "delete" => remove(cx, guild, self.subject.as_deref()).await,
            "priority" => rank(cx, guild, self.subject.as_deref(), self.target.as_deref()).await,
            "allow" => write(cx, guild, Effect::Allow, &self).await,
            "deny" => write(cx, guild, Effect::Deny, &self).await,
            _ => Err(Error::empty().title("invalid action")),
        }
    }
}

async fn list(cx: &Cx, guild: u64) -> Result<Response> {
    Ok(Response::embed(ui::listing(
        &store::all(cx.pool(), guild).await?,
    )))
}

async fn clear(cx: &Cx, guild: u64) -> Result<Response> {
    store::clear(cx.pool(), guild).await?;
    cx.app.permits.forget(guild);

    Ok(Response::embed(
        Embed::new("PERMISSION RULES CLEARED").tone(Tone::Danger),
    ))
}

async fn remove(cx: &Cx, guild: u64, id: Option<&str>) -> Result<Response> {
    let id = id
        .and_then(|raw| raw.trim_start_matches('#').parse::<i64>().ok())
        .ok_or_else(|| Error::empty().title("permission rule not found"))?;

    if !store::remove(cx.pool(), guild, id).await? {
        return Err(Error::empty().title("permission rule not found"));
    }

    cx.app.permits.forget(guild);

    Ok(Response::embed(
        Embed::new("PERMISSION RULE REMOVED").tone(Tone::Danger),
    ))
}

async fn scoped(cx: &Cx, raw: Option<&str>) -> Result<(Scope, Snowflake)> {
    let raw = raw.ok_or_else(|| {
        Error::empty()
            .title("missing role, channel or member")
            .with_hint("id, role:<id>, channel:<id> or member:<id>")
    })?;

    if let Some(said) = rule::subject(raw) {
        return Ok(said);
    }

    let id = raw.parse::<Snowflake>().map_err(|_| {
        Error::empty()
            .title("expected role, channel or member")
            .with_hint("id, role:<id>, channel:<id> or member:<id>")
    })?;

    if cx.guild().await?.roles.contains_key(&RoleId::new(id)) {
        return Ok((Scope::Role, id));
    }

    let known = cx
        .ctx
        .cache
        .guild(cx.guild_id()?)
        .is_some_and(|guild| guild.channels.contains_key(&ChannelId::new(id)));

    match known {
        true => Ok((Scope::Channel, id)),
        false => Ok((Scope::Member, id)),
    }
}

fn targeted(cx: &Cx, raw: Option<&str>) -> Result<Target> {
    let raw = raw.ok_or_else(|| Error::empty().title("missing command, category or '*'"))?;

    if raw == "*" {
        return Ok(Target::Everything);
    }

    if let Some(name) = raw.strip_prefix('@') {
        return rule::category(name)
            .map(Target::Category)
            .ok_or_else(|| Error::empty().title("category not found"));
    }

    let developer = cx.app.is_developer(cx.author_id().get());
    let found = cx
        .app
        .registry
        .find(raw)
        .filter(|entry| developer || !entry.meta.developer);

    if let Some(entry) = found {
        return Ok(Target::Command(entry.meta.name.to_string()));
    }

    rule::category(raw)
        .map(Target::Category)
        .ok_or_else(|| Error::empty().title("command not found"))
}

async fn write(cx: &Cx, guild: u64, effect: Effect, args: &Perms) -> Result<Response> {
    let (scope, subject) = scoped(cx, args.subject.as_deref()).await?;

    let target = targeted(cx, args.target.as_deref())?;

    let priority = args.priority.unwrap_or_default();
    let id = store::add(cx.pool(), guild, scope, subject, &target, effect, priority).await?;

    cx.app.permits.forget(guild);

    Ok(Response::embed(ui::written(&Rule {
        id,
        scope,
        subject,
        target,
        effect,
        priority,
    })))
}

async fn rank(cx: &Cx, guild: u64, id: Option<&str>, raw: Option<&str>) -> Result<Response> {
    let id = id
        .and_then(|raw| raw.trim_start_matches('#').parse::<i64>().ok())
        .ok_or_else(|| Error::empty().title("permission rule not found"))?;

    let priority = raw
        .and_then(|raw| raw.parse::<i32>().ok())
        .ok_or_else(|| Error::empty().title("invalid priority"))?;

    let moved = store::set_priority(cx.pool(), guild, id, priority)
        .await?
        .ok_or_else(|| Error::empty().title("permission rule not found"))?;

    cx.app.permits.forget(guild);

    Ok(Response::embed(ui::ranked(&moved)))
}
