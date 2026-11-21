mod condition;
mod matcher;
mod outcome;
mod secs;
mod source;

pub use condition::{Cmp, Condition, Measure};
pub use matcher::Matcher;
pub use outcome::{Notify, Outcome, Threshold};
pub use source::Source;

use std::fmt::{self, Display};

use chrono::{DateTime, Duration as ChronoDuration, Utc};
use serde::{Deserialize, Serialize};
use serenity::all::Permissions;
use sha2::{Digest, Sha256};

use crate::domain::Snowflake;
use crate::domain::ids::RuleId;

pub fn account_age(created_at: DateTime<Utc>) -> ChronoDuration {
    (Utc::now() - created_at).max(ChronoDuration::zero())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Mode {
    Disabled,
    Active,
}

impl Mode {
    pub fn as_str(&self) -> &'static str {
        match self {
            Mode::Disabled => "disabled",
            Mode::Active => "active",
        }
    }

    pub fn parse(raw: &str) -> Option<Self> {
        match raw {
            "disabled" => Some(Mode::Disabled),
            "active" => Some(Mode::Active),
            _ => None,
        }
    }
}

impl Display for Mode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Body {
    #[serde(default)]
    pub sources: Vec<Source>,
    #[serde(default)]
    pub matches: Vec<Matcher>,
    #[serde(default)]
    pub nevers: Vec<Matcher>,
    #[serde(default)]
    pub conditions: Vec<Condition>,
    #[serde(default)]
    pub only: Vec<Snowflake>,
    #[serde(default)]
    pub ignore_channels: Vec<Snowflake>,
    #[serde(default)]
    pub ignore_roles: Vec<Snowflake>,
    #[serde(default)]
    pub ignore_users: Vec<Snowflake>,
    #[serde(default)]
    pub ignore_permissions: Permissions,
    #[serde(default)]
    pub outcome: Outcome,
    #[serde(default)]
    pub after: Option<Threshold>,
}

impl Body {
    pub fn sources(&self) -> &[Source] {
        match self.sources.is_empty() {
            true => &[
                Source::Content,
                Source::Image,
                Source::Filename,
                Source::Embed,
            ],
            false => &self.sources,
        }
    }

    pub fn has_source(&self, source: Source) -> bool {
        self.sources().contains(&source)
    }

    pub fn ignores_permissions(&self) -> bool {
        !self.ignore_permissions.is_empty()
    }

    pub fn windows(&self) -> impl Iterator<Item = Option<ChronoDuration>> {
        self.conditions
            .iter()
            .filter(|condition| condition.measure.counts_record())
            .map(|condition| condition.window)
    }

    pub fn hash(&self) -> String {
        let mut hasher = Sha256::new();

        for source in self.sources() {
            hasher.update(source.as_str().as_bytes());
            hasher.update(b"\x1f");
        }

        hasher.update(b"\x1e");

        for matcher in &self.matches {
            hasher.update(matcher.render().as_bytes());
            hasher.update(b"\x1f");
        }

        hasher.update(b"\x1e");

        for matcher in &self.nevers {
            hasher.update(matcher.render().as_bytes());
            hasher.update(b"\x1f");
        }

        crate::platform::text::hex(&hasher.finalize())
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Author {
    #[default]
    Guild,
    Developers,
}

#[derive(Clone, Debug)]
pub struct Rule {
    pub id: RuleId,
    pub guild: Snowflake,
    pub name: String,
    pub mode: Mode,
    pub author: Author,
    pub source: String,
    pub body: Body,
}

impl Rule {
    pub fn hash(&self) -> String {
        self.body.hash()
    }
}
