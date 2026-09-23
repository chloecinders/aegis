use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration as Wait;

use std::cmp::Reverse;

use chrono::{DateTime, Duration, Utc};
use serenity::all::{CacheHttp, GuildId};
use serenity::nonmax::NonMaxU8;

use crate::domain::Snowflake;
use crate::features::guildlog::attribution::Attribution;
use crate::platform::cache::Cache;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Candidate {
    pub entry: Snowflake,
    pub actor: Snowflake,
    pub target: Snowflake,
    pub channel: Snowflake,
    pub count: u64,
    pub created: DateTime<Utc>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Thread {
    pub guild: Snowflake,
    pub actor: Snowflake,
    pub target: Snowflake,
    pub channel: Snowflake,
}

#[derive(Clone, Copy, Debug)]
struct Seen {
    entry: Snowflake,
    count: u64,
}

pub struct Count {
    seen: Mutex<HashMap<Thread, Seen>>,
}

impl Default for Count {
    fn default() -> Self {
        Self::new()
    }
}

impl Count {
    pub fn new() -> Self {
        Self {
            seen: Mutex::new(HashMap::new()),
        }
    }

    pub fn len(&self) -> usize {
        self.seen.lock().map(|seen| seen.len()).unwrap_or_default()
    }

    pub fn claim(
        &self,
        guild: Snowflake,
        target: Snowflake,
        channel: Snowflake,
        entries: &[Candidate],
        now: DateTime<Utc>,
    ) -> Attribution {
        let Ok(mut seen) = self.seen.lock() else {
            return Attribution::Unknown;
        };

        let fresh = |entry: &Candidate| now - entry.created < Duration::seconds(600);

        let mut ranked: Vec<&Candidate> = entries
            .iter()
            .filter(|entry| entry.target == target && entry.channel == channel && fresh(entry))
            .collect();

        ranked.sort_unstable_by_key(|one| Reverse(one.created));

        for entry in ranked {
            let thread = Thread {
                guild,
                actor: entry.actor,
                target,
                channel,
            };

            let advanced = match seen.get(&thread) {
                Some(known) if known.entry == entry.entry => entry.count > known.count,
                Some(_) => true,
                None => entry.count > 0,
            };

            if !advanced {
                continue;
            }

            if seen.len() >= 4096 && !seen.contains_key(&thread) {
                shed(&mut seen);
            }

            seen.insert(
                thread,
                Seen {
                    entry: entry.entry,
                    count: entry.count,
                },
            );

            return Attribution::Polled(entry.actor);
        }

        Attribution::Unknown
    }

    fn shed(&self) {
        if let Ok(mut seen) = self.seen.lock() {
            shed(&mut seen);
        }
    }

    pub fn sweep(&self) {
        if self.len() >= 4096 {
            self.shed();
        }
    }
}

fn shed(seen: &mut HashMap<Thread, Seen>) {
    let keep = seen.len() / 2;
    let doomed: Vec<Thread> = seen.keys().take(seen.len() - keep).copied().collect();

    for thread in doomed {
        seen.remove(&thread);
    }
}

pub type Page = Arc<Vec<Candidate>>;

pub struct Reader {
    pages: Cache<Snowflake, Page>,
}

impl Default for Reader {
    fn default() -> Self {
        Self::new()
    }
}

impl Reader {
    pub fn new() -> Self {
        Self {
            pages: Cache::new(4096, Some(Wait::from_millis(2000))),
        }
    }

    pub async fn recent(&self, http: impl CacheHttp, guild: Snowflake) -> Page {
        if let Some(known) = self.pages.get(&guild) {
            return known;
        }

        let fetched = fetch(http, guild).await;

        self.pages.insert(guild, Arc::clone(&fetched));

        fetched
    }

    pub fn forget(&self, guild: Snowflake) {
        self.pages.remove(&guild);
    }
}

async fn fetch(http: impl CacheHttp, guild: Snowflake) -> Page {
    use serenity::all::audit_log::{Action, MessageAction};

    let read = GuildId::new(guild)
        .audit_logs(
            http.http(),
            Some(Action::Message(MessageAction::Delete)),
            None,
            None,
            None,
            NonMaxU8::new(25),
        )
        .await;

    let Ok(logs) = read else {
        return Arc::new(Vec::new());
    };

    Arc::new(logs.entries.iter().filter_map(candidate).collect())
}

fn candidate(entry: &serenity::all::AuditLogEntry) -> Option<Candidate> {
    let options = entry.options.as_ref()?;

    Some(Candidate {
        entry: entry.id.get(),
        actor: entry.user_id?.get(),
        target: entry.target_id?.get(),
        channel: options.channel_id?.get(),
        count: options.count.map_or(1, |count| count.get()),
        created: *entry.id.created_at(),
    })
}
