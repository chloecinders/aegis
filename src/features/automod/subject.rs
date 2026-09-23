use serenity::all::{Context, GuildId, Permissions, RoleId, UserId};

use crate::command::error::Result;
use crate::domain::Snowflake;
use crate::features::automod::eval::{Observed, Record};
use crate::features::automod::rule::{Rule, Source};
use crate::features::automod::sources;
use crate::features::records;
use crate::platform::discord::fetch;
use crate::platform::discord::permissions::Actor;
use crate::platform::text::fuzzy::Haystack;

pub struct Fixed<'a> {
    pub channel: Snowflake,
    pub roles: &'a [Snowflake],
    pub permissions: Permissions,
    pub age: chrono::Duration,
    pub counts: sources::Counts,
    pub record: Option<&'a Record>,
}

impl<'a> Fixed<'a> {
    pub fn observed(&self, source: Source, read: Haystack<'a>) -> Observed<'a> {
        Observed {
            source,
            read,
            channel: self.channel,
            roles: self.roles,
            permissions: self.permissions,
            age: self.age,
            mentions: self.counts.mentions,
            links: self.counts.links,
            invites: self.counts.invites,
            attachments: self.counts.attachments,
            animated: self.counts.animated,
            record: self.record,
        }
    }
}

pub async fn wielded<'a>(
    ctx: &Context,
    rules: impl IntoIterator<Item = &'a Rule>,
    guild: Snowflake,
    member: UserId,
    roles: &[Snowflake],
) -> Result<Permissions> {
    if !rules
        .into_iter()
        .any(|rule| rule.body.ignores_permissions())
    {
        return Ok(Permissions::empty());
    }

    let role_ids: Vec<RoleId> = roles.iter().map(|role| RoleId::new(*role)).collect();
    let snapshot = fetch::snapshot(ctx, GuildId::new(guild)).await?;
    let granted = snapshot.base(Actor {
        id: member,
        roles: &role_ids,
    });

    match granted.contains(Permissions::ADMINISTRATOR) {
        true => Ok(Permissions::all()),
        false => Ok(granted),
    }
}

pub async fn record_of<'a>(
    app: &crate::app::App,
    rules: impl IntoIterator<Item = &'a Rule>,
    guild: Snowflake,
    member: Snowflake,
) -> Result<Option<Record>> {
    let windows: Vec<Option<chrono::Duration>> = rules
        .into_iter()
        .flat_map(|rule| rule.body.windows())
        .collect();

    if windows.is_empty() {
        return Ok(None);
    }

    let since = match windows.iter().any(Option::is_none) {
        true => None,
        false => windows
            .iter()
            .flatten()
            .max()
            .map(|widest| chrono::Utc::now() - *widest),
    };

    Ok(Some(Record {
        punishments: records::store::count(&app.pool, guild, member, since).await?,
    }))
}
