pub mod controls;
pub mod store;
pub mod ui;

use crate::domain::Snowflake;
use crate::domain::ids::RuleId;
use crate::features::automod::rule::{Author, Body, Mode, Rule};

pub fn generate() -> RuleId {
    RuleId::from(format!("*{}", RuleId::generate()))
}

#[derive(Clone, Debug)]
pub struct Managed {
    pub id: RuleId,
    pub name: String,
    pub description: String,
    pub mode: Mode,
    pub source: String,
    pub body: Body,
}

#[derive(Clone, Debug)]
pub struct Subscription {
    pub rule: RuleId,
    pub guild: Snowflake,
    pub mode: Mode,
    pub written: String,
    pub response: Body,
}

pub struct Offer {
    pub managed: Managed,
    pub subscription: Option<Subscription>,
}

impl Offer {
    pub fn effective(&self) -> Mode {
        match &self.subscription {
            Some(subscription) => self.managed.mode.min(subscription.mode),
            None => Mode::Disabled,
        }
    }
}

pub fn combine(managed: &Managed, subscription: &Subscription) -> Rule {
    let body = Body {
        sources: managed.body.sources.clone(),
        matches: managed.body.matches.clone(),
        nevers: managed.body.nevers.clone(),
        conditions: managed.body.conditions.clone(),
        only: subscription.response.only.clone(),
        ignore_channels: subscription.response.ignore_channels.clone(),
        ignore_roles: subscription.response.ignore_roles.clone(),
        ignore_users: subscription.response.ignore_users.clone(),
        ignore_permissions: subscription.response.ignore_permissions,
        outcome: subscription.response.outcome.clone(),
        after: subscription.response.after,
    };

    Rule {
        id: managed.id.clone(),
        guild: subscription.guild,
        name: managed.name.clone(),
        mode: managed.mode.min(subscription.mode),
        author: Author::Developers,
        source: String::new(),
        body,
    }
}
