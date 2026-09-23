use std::fmt::{self, Display};

use serenity::all::{CacheHttp, UserId};

use crate::domain::Snowflake;
use crate::platform::discord::fetch;
use crate::platform::ui::embed::{code, mention};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Attribution {
    Bot(Snowflake),
    Gateway(Snowflake),
    Polled(Snowflake),
    Unknown,
}

impl Attribution {
    pub fn actor(&self) -> Option<Snowflake> {
        match self {
            Attribution::Bot(actor) | Attribution::Gateway(actor) | Attribution::Polled(actor) => {
                Some(*actor)
            }
            Attribution::Unknown => None,
        }
    }

    pub fn confidence(&self) -> u8 {
        match self {
            Attribution::Bot(_) => 3,
            Attribution::Gateway(_) => 2,
            Attribution::Polled(_) => 1,
            Attribution::Unknown => 0,
        }
    }

    pub fn is_resolved(&self) -> bool {
        !matches!(self, Attribution::Unknown)
    }

    pub fn wants_poll(&self) -> bool {
        matches!(self, Attribution::Unknown)
    }

    pub fn or(self, other: Attribution) -> Attribution {
        match other.confidence() > self.confidence() {
            true => other,
            false => self,
        }
    }

    pub fn line(&self, bot: Snowflake, name: Option<&str>) -> Option<String> {
        let actor = self.actor()?;
        let display = match actor == bot {
            true => code("Aegis"),
            false => mention(actor, name),
        };

        Some(format!("Actor: {display}"))
    }
}

impl Display for Attribution {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Attribution::Bot(_) => "self",
            Attribution::Gateway(_) => "gateway",
            Attribution::Polled(_) => "polled",
            Attribution::Unknown => "unknown",
        })
    }
}

pub async fn username(http: impl CacheHttp, actor: Attribution, bot: Snowflake) -> Option<String> {
    let actor = actor.actor().filter(|id| *id != bot)?;

    fetch::user(http, UserId::new(actor))
        .await
        .ok()
        .map(|found| found.name.to_string())
}
