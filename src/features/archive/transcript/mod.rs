pub mod store;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::domain::Snowflake;
use crate::domain::ids::TranscriptId;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Scope {
    Channel,
    User,
    Cleared,
    Selection,
}

impl Scope {
    pub fn as_str(&self) -> &'static str {
        match self {
            Scope::Channel => "channel",
            Scope::User => "user",
            Scope::Cleared => "cleared",
            Scope::Selection => "selection",
        }
    }

    pub fn parse(raw: &str) -> Option<Self> {
        match raw {
            "channel" => Some(Scope::Channel),
            "user" => Some(Scope::User),
            "cleared" => Some(Scope::Cleared),
            "selection" => Some(Scope::Selection),
            _ => None,
        }
    }

    pub fn spans_channels(&self) -> bool {
        matches!(self, Scope::User | Scope::Cleared | Scope::Selection)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Meta {
    pub id: TranscriptId,
    pub guild: Snowflake,
    pub scope: Scope,
    pub channel: Option<Snowflake>,
    pub channel_name: Option<String>,
    pub subject: Option<Snowflake>,
    pub subject_name: Option<String>,
    pub window_start: Option<DateTime<Utc>>,
    pub window_end: Option<DateTime<Utc>>,
    pub moderator_name: String,
    pub created_at: DateTime<Utc>,
    pub total: i64,
}

impl Meta {
    pub fn title(&self) -> String {
        match self.scope {
            Scope::Channel => match &self.channel_name {
                Some(name) => format!("#{name}"),
                None => String::from("deleted channel"),
            },
            Scope::User | Scope::Cleared => match (&self.subject_name, self.subject) {
                (Some(name), _) => format!("messages from {name}"),
                (None, Some(subject)) => format!("messages from {subject}"),
                (None, None) => String::from("messages from a member"),
            },
            Scope::Selection => format!("{} purged messages", self.total),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Request {
    pub guild: Snowflake,
    pub scope: Scope,
    pub channel: Option<Snowflake>,
    pub channel_name: Option<String>,
    pub subject: Option<Snowflake>,
    pub subject_name: Option<String>,
    pub window_start: Option<DateTime<Utc>>,
    pub window_end: Option<DateTime<Utc>>,
    pub moderator_name: String,
    pub selected: Vec<Snowflake>,
}

impl Request {
    pub fn channel(
        guild: Snowflake,
        channel: Snowflake,
        name: Option<String>,
        moderator_name: String,
    ) -> Self {
        Self {
            guild,
            scope: Scope::Channel,
            channel: Some(channel),
            channel_name: name,
            subject: None,
            subject_name: None,
            window_start: None,
            window_end: None,
            moderator_name,
            selected: Vec::new(),
        }
    }

    pub fn cleared(
        guild: Snowflake,
        subject: Snowflake,
        name: String,
        since: DateTime<Utc>,
        moderator_name: String,
    ) -> Self {
        Self {
            guild,
            scope: Scope::Cleared,
            channel: None,
            channel_name: None,
            subject: Some(subject),
            subject_name: Some(name),
            window_start: Some(since),
            window_end: Some(Utc::now()),
            moderator_name,
            selected: Vec::new(),
        }
    }

    pub fn history(
        guild: Snowflake,
        subject: Snowflake,
        name: String,
        moderator_name: String,
    ) -> Self {
        Self {
            guild,
            scope: Scope::User,
            channel: None,
            channel_name: None,
            subject: Some(subject),
            subject_name: Some(name),
            window_start: None,
            window_end: None,
            moderator_name,
            selected: Vec::new(),
        }
    }

    pub fn selection(guild: Snowflake, chosen: Vec<Snowflake>, moderator_name: String) -> Self {
        Self {
            guild,
            scope: Scope::Selection,
            channel: None,
            channel_name: None,
            subject: None,
            subject_name: None,
            window_start: None,
            window_end: None,
            moderator_name,
            selected: chosen,
        }
    }

    pub fn is_answerable(&self) -> bool {
        match self.scope {
            Scope::Channel => self.channel.is_some(),
            Scope::User | Scope::Cleared => self.subject.is_some(),
            Scope::Selection => !self.selected.is_empty(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Page<T> {
    pub messages: Vec<T>,
    pub next: Option<Snowflake>,
}

impl<T> Page<T> {
    pub fn of(messages: Vec<T>, cursor: impl Fn(&T) -> Snowflake, limit: i64) -> Self {
        let next = match messages.len() as i64 >= limit {
            true => messages.first().map(&cursor),
            false => None,
        };

        Self { messages, next }
    }
}

pub fn url(base: Option<&str>, guild: Snowflake, id: &TranscriptId) -> Option<String> {
    let base = base?.trim_end_matches('/');

    match base.is_empty() {
        true => None,
        false => Some(format!("{base}/transcript/{guild}/{id}")),
    }
}

pub fn limit(asked: Option<i64>) -> i64 {
    asked.unwrap_or(200).clamp(1, 200)
}
