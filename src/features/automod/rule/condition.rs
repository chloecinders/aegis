use chrono::Duration as ChronoDuration;
use serde::{Deserialize, Serialize};

use crate::domain::punishment::PunishmentType;
use crate::platform::text::duration;

use super::secs;
use super::source::Source;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Measure {
    Mentions,
    Links,
    Invites,
    Attachments,
    Animated,
    AccountAge,
    Warns,
    Mutes,
    Kicks,
    Bans,
    Punishments,
}

impl Measure {
    pub fn as_str(&self) -> &'static str {
        match self {
            Measure::Mentions => "mentions",
            Measure::Links => "links",
            Measure::Invites => "invites",
            Measure::Attachments => "attachments",
            Measure::Animated => "animated",
            Measure::AccountAge => "account",
            Measure::Warns => "warns",
            Measure::Mutes => "mutes",
            Measure::Kicks => "kicks",
            Measure::Bans => "bans",
            Measure::Punishments => "punishments",
        }
    }

    pub fn parse(raw: &str) -> Option<Self> {
        match raw {
            "mentions" => Some(Measure::Mentions),
            "links" => Some(Measure::Links),
            "invites" => Some(Measure::Invites),
            "attachments" => Some(Measure::Attachments),
            "animated" => Some(Measure::Animated),
            "account" => Some(Measure::AccountAge),
            "warns" => Some(Measure::Warns),
            "mutes" => Some(Measure::Mutes),
            "kicks" => Some(Measure::Kicks),
            "bans" => Some(Measure::Bans),
            "punishments" => Some(Measure::Punishments),
            _ => None,
        }
    }

    pub fn counts_record(&self) -> bool {
        matches!(
            self,
            Measure::Warns | Measure::Mutes | Measure::Kicks | Measure::Bans | Measure::Punishments
        )
    }

    pub fn tallies(&self, verb: PunishmentType) -> bool {
        match self {
            Measure::Warns => matches!(verb, PunishmentType::Warn),
            Measure::Mutes => matches!(verb, PunishmentType::Mute),
            Measure::Kicks => matches!(verb, PunishmentType::Kick),
            Measure::Bans => matches!(verb, PunishmentType::Ban),
            Measure::Punishments => !matches!(verb, PunishmentType::Unban | PunishmentType::Unmute),
            _ => false,
        }
    }

    pub fn available_on(&self, source: Source) -> bool {
        match self {
            Measure::AccountAge => true,
            measure if measure.counts_record() => true,
            _ => !matches!(source, Source::Join),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Cmp {
    Above,
    Below,
    AtLeast,
    AtMost,
}

impl Cmp {
    pub fn as_str(&self) -> &'static str {
        match self {
            Cmp::Above => ">",
            Cmp::Below => "<",
            Cmp::AtLeast => ">=",
            Cmp::AtMost => "<=",
        }
    }

    pub fn parse(raw: &str) -> Option<Self> {
        match raw {
            ">" => Some(Cmp::Above),
            "<" => Some(Cmp::Below),
            ">=" => Some(Cmp::AtLeast),
            "<=" => Some(Cmp::AtMost),
            _ => None,
        }
    }

    pub fn has(&self, observed: i64, bound: i64) -> bool {
        match self {
            Cmp::Above => observed > bound,
            Cmp::Below => observed < bound,
            Cmp::AtLeast => observed >= bound,
            Cmp::AtMost => observed <= bound,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Condition {
    pub measure: Measure,
    pub cmp: Cmp,
    pub bound: i64,
    #[serde(default, with = "secs::maybe")]
    pub window: Option<ChronoDuration>,
}

impl Condition {
    pub fn render(&self) -> String {
        if self.measure.counts_record() {
            let within = match self.window {
                Some(window) => format!(" in {}", duration::compact(window)),
                None => String::new(),
            };

            return format!(
                "when {} {} {}{within}",
                self.measure.as_str(),
                self.cmp.as_str(),
                self.bound
            );
        }

        if self.measure != Measure::AccountAge {
            return format!(
                "when {} {} {}",
                self.measure.as_str(),
                self.cmp.as_str(),
                self.bound
            );
        }

        let side = match self.cmp {
            Cmp::Below | Cmp::AtMost => "younger",
            Cmp::Above | Cmp::AtLeast => "older",
        };

        format!(
            "when account {side} than {}",
            duration::compact(ChronoDuration::seconds(self.bound))
        )
    }
}
