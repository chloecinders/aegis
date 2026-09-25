use std::time::Duration;

use serenity::all::{CacheHttp, UserId};

use crate::app::App;
use crate::domain::Snowflake;
use crate::features::guildlog::Posted;
use crate::features::guildlog::attribution::Attribution;
use crate::features::guildlog::member::Part;
use crate::platform::cache::Cache;
use crate::platform::discord::fetch;
use crate::platform::ui::embed::Embed;

pub type Key = (Snowflake, Snowflake, Snowflake);
pub type Trace = (Snowflake, Snowflake, Part);
pub type Bulk = (Snowflake, Snowflake);

#[derive(Clone, Debug)]
struct Waiting {
    at: Posted,
    embed: Embed,
}

#[derive(Clone, Debug)]
struct Update {
    at: Posted,
    embed: Embed,
    parts: Vec<Part>,
}

#[derive(Clone, Debug)]
struct AuditEntry {
    actor: Snowflake,
    reason: Option<String>,
}

#[derive(Clone, Debug)]
pub struct Claimed {
    pub actor: Attribution,
    pub reason: Option<String>,
}

pub struct Awaiting {
    waiting: Cache<Key, Waiting>,
    bulk: Cache<Bulk, Waiting>,
    updates: Cache<Trace, Update>,
    early_audits: Cache<Trace, AuditEntry>,
    early_bulk_audits: Cache<Bulk, Snowflake>,
}

impl Default for Awaiting {
    fn default() -> Self {
        Self::new()
    }
}

impl Awaiting {
    pub fn new() -> Self {
        Self {
            waiting: Cache::new(2048, Some(Duration::from_secs(30))),
            bulk: Cache::new(2048, Some(Duration::from_secs(30))),
            updates: Cache::new(2048, Some(Duration::from_secs(30))),
            early_audits: Cache::new(2048, Some(Duration::from_secs(10))),
            early_bulk_audits: Cache::new(2048, Some(Duration::from_secs(10))),
        }
    }

    pub fn expect(&self, key: Key, at: Posted, embed: &Embed, known: Attribution) {
        if known.is_resolved() {
            return;
        }

        self.waiting.insert(
            key,
            Waiting {
                at,
                embed: embed.clone(),
            },
        );
    }

    pub fn forget(&self, key: &Key) {
        self.waiting.remove(key);
    }

    pub fn expect_bulk(&self, key: Bulk, at: Posted, embed: &Embed, known: Attribution) {
        if known.is_resolved() {
            return;
        }

        self.bulk.insert(
            key,
            Waiting {
                at,
                embed: embed.clone(),
            },
        );
    }

    pub fn claim_bulk(&self, key: &Bulk) -> Attribution {
        match self.early_bulk_audits.remove(key) {
            Some(actor) => Attribution::Gateway(actor),
            None => Attribution::Unknown,
        }
    }

    pub fn track(
        &self,
        guild: Snowflake,
        target: Snowflake,
        parts: &[Part],
        at: Posted,
        embed: &Embed,
        complete: bool,
    ) {
        if complete {
            return;
        }

        for part in parts {
            self.updates.insert(
                (guild, target, *part),
                Update {
                    at,
                    embed: embed.clone(),
                    parts: parts.to_vec(),
                },
            );
        }
    }

    pub fn claim(&self, guild: Snowflake, target: Snowflake, parts: &[Part]) -> Claimed {
        let mut found = Claimed {
            actor: Attribution::Unknown,
            reason: None,
        };

        for part in parts {
            let Some(audit) = self.early_audits.remove(&(guild, target, *part)) else {
                continue;
            };

            if found.reason.is_some() && audit.reason.is_none() {
                continue;
            }

            found = Claimed {
                actor: Attribution::Gateway(audit.actor),
                reason: audit.reason,
            };
        }

        found
    }

    pub fn sweep(&self) {
        self.waiting.sweep();
        self.bulk.sweep();
        self.updates.sweep();
        self.early_audits.sweep();
        self.early_bulk_audits.sweep();
    }
}

pub async fn attribute_deletion(
    app: &App,
    http: impl CacheHttp,
    key: Key,
    actor: Snowflake,
    bot: Snowflake,
) -> bool {
    let Some(waiting) = app.awaiting.waiting.remove(&key) else {
        return false;
    };

    attribute(app, http, waiting, actor, bot).await;

    true
}

pub async fn attribute_bulk(
    app: &App,
    http: impl CacheHttp,
    key: Bulk,
    actor: Snowflake,
    bot: Snowflake,
) {
    let Some(waiting) = app.awaiting.bulk.remove(&key) else {
        app.awaiting.early_bulk_audits.insert(key, actor);

        return;
    };

    attribute(app, http, waiting, actor, bot).await;
}

async fn attribute(
    app: &App,
    http: impl CacheHttp,
    waiting: Waiting,
    actor: Snowflake,
    bot: Snowflake,
) {
    let name = fetch::user(&http, UserId::new(actor))
        .await
        .ok()
        .map(|found| found.name);
    let amended = attach(
        &waiting.embed,
        &Attribution::Gateway(actor),
        name.as_deref(),
        bot,
    );

    if let Err(failure) =
        super::store::attribute(&app.pool, waiting.at.message.get(), Some(actor)).await
    {
        app.reporter.record(&failure, Default::default());
    }

    if let Err(failure) = super::rewrite(http, waiting.at, &amended).await {
        app.reporter.record(&failure, Default::default());
    }
}

pub async fn attribute_update(
    app: &App,
    http: impl CacheHttp,
    guild: Snowflake,
    target: Snowflake,
    parts: &[Part],
    actor: Snowflake,
    reason: Option<&str>,
    bot: Snowflake,
) {
    let mut attributed: Vec<Part> = Vec::new();
    let mut amending = Vec::new();

    for part in parts {
        if attributed.contains(part) {
            continue;
        }

        let Some(update) = app.awaiting.updates.remove(&(guild, target, *part)) else {
            app.awaiting.early_audits.insert(
                (guild, target, *part),
                AuditEntry {
                    actor,
                    reason: reason.map(str::to_owned),
                },
            );
            continue;
        };

        for sibling in &update.parts {
            app.awaiting.updates.remove(&(guild, target, *sibling));
        }

        attributed.extend(update.parts.iter().copied());
        amending.push(update);
    }

    let resolved = Attribution::Gateway(actor);
    let name = fetch::user(&http, UserId::new(actor))
        .await
        .ok()
        .map(|found| found.name);

    for update in amending {
        if actor == target
            && update
                .parts
                .iter()
                .all(|part| matches!(part, Part::Gained(_) | Part::Lost(_)))
        {
            if let Err(failure) = super::retract(app, &http, update.at).await {
                app.reporter.record(&failure, Default::default());
            }

            continue;
        }

        let mut amended = attach(&update.embed, &resolved, name.as_deref(), bot);

        if let Some(given) = reason {
            amended = amended.quote(given);
        }

        if let Err(failure) =
            super::store::attribute(&app.pool, update.at.message.get(), Some(actor)).await
        {
            app.reporter.record(&failure, Default::default());
        }

        if let Err(failure) = super::rewrite(&http, update.at, &amended).await {
            app.reporter.record(&failure, Default::default());
        }

        if reason.is_some() {
            continue;
        }

        for sibling in update.parts.iter().filter(|part| !parts.contains(part)) {
            app.awaiting.updates.insert(
                (guild, target, *sibling),
                Update {
                    at: update.at,
                    embed: amended.clone(),
                    parts: update.parts.clone(),
                },
            );
        }
    }
}

pub fn attach(embed: &Embed, actor: &Attribution, name: Option<&str>, bot: Snowflake) -> Embed {
    let Some(line) = actor.line(bot, name) else {
        return embed.clone();
    };

    let mut amended = embed.clone();

    if amended
        .subtitles
        .iter()
        .any(|part| part.starts_with("Actor:"))
    {
        return amended;
    }

    amended.subtitles.push(line);
    amended
}
