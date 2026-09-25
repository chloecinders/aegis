use std::collections::HashMap;
use std::sync::Mutex;
use std::time::Instant;

use chrono::{DateTime, Duration, Utc};
use serenity::all::Permissions;

use crate::domain::Snowflake;
use crate::domain::ids::RuleId;
use crate::domain::punishment::{Punishment, PunishmentType};
use crate::features::automod::rule::{Author, Condition, Measure, Mode, Outcome, Rule, Source};
use crate::platform::text::fuzzy::Haystack;
use crate::platform::text::{duration, truncate};

#[derive(Clone, Debug, Default)]
pub struct Record {
    pub punishments: Vec<(PunishmentType, DateTime<Utc>)>,
}

impl Record {
    pub fn count(&self, measure: Measure, window: Option<Duration>) -> i64 {
        let cutoff = window.map(|window| Utc::now() - window);

        self.punishments
            .iter()
            .filter(|(verb, at)| measure.counts(*verb) && cutoff.is_none_or(|cutoff| *at >= cutoff))
            .count() as i64
    }
}

#[derive(Clone, Debug, Default)]
pub struct Observed<'a> {
    pub source: Source,
    pub read: Haystack<'a>,
    pub channel: Snowflake,
    pub roles: &'a [Snowflake],
    pub permissions: Permissions,
    pub age: Duration,
    pub mentions: i64,
    pub links: i64,
    pub invites: i64,
    pub attachments: i64,
    pub animated: i64,
    pub record: Option<&'a Record>,
}

impl Observed<'_> {
    pub fn read(&self, condition: &Condition) -> i64 {
        match condition.measure {
            Measure::Mentions => self.mentions,
            Measure::Links => self.links,
            Measure::Invites => self.invites,
            Measure::Attachments => self.attachments,
            Measure::Animated => self.animated,
            Measure::AccountAge => self.age.num_seconds(),
            measure => self
                .record
                .map_or(0, |record| record.count(measure, condition.window)),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Hit {
    pub rule: RuleId,
    pub name: String,
    pub source: Source,
    pub clause: Option<String>,
    pub counted: Option<String>,
    pub excerpt: String,
    pub mode: Mode,
    pub author: Author,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Miss {
    Source,
    Channel,
    Exempt,
    Condition,
    NoMatch,
    Excluded,
}

pub fn evaluate(rule: &Rule, observed: &Observed) -> Result<Hit, Miss> {
    if !rule.body.has_source(observed.source) {
        return Err(Miss::Source);
    }

    if !rule.body.only.is_empty() && !rule.body.only.contains(&observed.channel) {
        return Err(Miss::Channel);
    }

    if rule.body.ignore_channels.contains(&observed.channel) {
        return Err(Miss::Channel);
    }

    if rule
        .body
        .ignore_roles
        .iter()
        .any(|role| observed.roles.contains(role))
    {
        return Err(Miss::Exempt);
    }

    if observed
        .permissions
        .intersects(rule.body.ignore_permissions)
    {
        return Err(Miss::Exempt);
    }

    let has = rule
        .body
        .conditions
        .iter()
        .all(|condition| condition.cmp.has(observed.read(condition), condition.bound));

    if !has {
        return Err(Miss::Condition);
    }

    let tallied = || {
        let counted: Vec<String> = rule
            .body
            .conditions
            .iter()
            .filter(|condition| condition.measure.counts_record())
            .map(|condition| match condition.window {
                Some(window) => format!(
                    "{} {} in {}",
                    observed.read(condition),
                    condition.measure.as_str(),
                    duration::compact(window)
                ),
                None => format!(
                    "{} {}",
                    observed.read(condition),
                    condition.measure.as_str()
                ),
            })
            .collect();

        match counted.is_empty() {
            true => None,
            false => Some(counted.join(", ")),
        }
    };

    let hit = |clause: Option<String>| {
        Ok(Hit {
            rule: rule.id.clone(),
            name: rule.name.clone(),
            source: observed.source,
            clause,
            counted: tallied(),
            excerpt: truncate::clamp(observed.read.text().trim(), 240),
            mode: rule.mode,
            author: rule.author,
        })
    };

    if rule.body.matches.is_empty() {
        return hit(None);
    }

    let Some(matched) = rule
        .body
        .matches
        .iter()
        .find(|matcher| matcher.test(&observed.read))
    else {
        return Err(Miss::NoMatch);
    };

    if rule
        .body
        .nevers
        .iter()
        .any(|never| never.test(&observed.read))
    {
        return Err(Miss::Excluded);
    }

    hit(Some(matched.render()))
}

pub fn severest<'a>(hits: &'a [Hit], rules: &'a [Rule]) -> Option<(&'a Hit, &'a Rule)> {
    hits.iter()
        .filter(|hit| hit.mode == Mode::Active)
        .filter_map(|hit| {
            rules
                .iter()
                .find(|rule| rule.id == hit.rule)
                .map(|rule| (hit, rule))
        })
        .filter(|(_, rule)| rule.body.outcome.acts())
        .max_by_key(|(_, rule)| rule.body.outcome.severity())
}

pub fn punishment(
    outcome: &Outcome,
    guild: Snowflake,
    bot: Snowflake,
    target: Snowflake,
) -> Option<Punishment> {
    let verb = outcome.punishment_type?;
    let reason = outcome
        .reason
        .clone()
        .unwrap_or_else(|| String::from("Automod"));

    Some(
        Punishment::new(verb, guild, bot, target)
            .reason(crate::domain::reason::Reason::new(&reason))
            .duration(match verb.has_duration() {
                true => outcome.duration,
                false => Duration::zero(),
            })
            .clear_days(outcome.clear_days),
    )
}

pub struct Strikes {
    seen: Mutex<HashMap<(RuleId, Snowflake, Snowflake), Vec<Instant>>>,
    capacity: usize,
}

impl Strikes {
    pub fn new(capacity: usize) -> Self {
        Self {
            seen: Mutex::new(HashMap::new()),
            capacity,
        }
    }

    pub fn record(
        &self,
        rule: &RuleId,
        guild: Snowflake,
        member: Snowflake,
        window: Duration,
    ) -> u32 {
        let Ok(mut seen) = self.seen.lock() else {
            return 1;
        };

        let cutoff = window.to_std().unwrap_or_default();
        let now = Instant::now();
        let key = (rule.clone(), guild, member);

        if seen.len() >= self.capacity && !seen.contains_key(&key) {
            seen.retain(|_, hits| hits.iter().any(|at| now.duration_since(*at) < cutoff));
        }

        let hits = seen.entry(key).or_default();

        hits.retain(|at| now.duration_since(*at) < cutoff);
        hits.push(now);

        hits.len() as u32
    }
}
