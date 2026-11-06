use chrono::{DateTime, Utc};
use serenity::all::{CacheHttp, UserId};

use crate::app::App;
use crate::command::error::Result;
use crate::domain::Snowflake;
use crate::domain::logtype::LogType;
use crate::domain::punishment::{Punishment, PunishmentState, PunishmentType};
use crate::domain::reason::Reason;
use crate::features::guildlog;
use crate::features::punishments::store;
use crate::features::records::store as records;
use crate::platform::discord::fetch;
use crate::platform::ui::punishment as ui;

#[derive(Clone, Copy, Debug)]
pub struct Involved {
    pub guild: Snowflake,
    pub actor: Snowflake,
    pub target: Snowflake,
    pub bot: Snowflake,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    Applied(DateTime<Utc>),
    Ended,
    Nothing,
}

pub fn read(
    before: Option<DateTime<Utc>>,
    after: Option<DateTime<Utc>>,
    reason: Option<&str>,
    by_us: bool,
    now: DateTime<Utc>,
) -> Outcome {
    if by_us || reason.is_some_and(|given| given.contains("Aegis Managed")) || before == after {
        return Outcome::Nothing;
    }

    let was_muted = before.is_some_and(|until| until > now);

    match after {
        Some(until) if until > now => Outcome::Applied(until),
        _ => match was_muted {
            true => Outcome::Ended,
            false => Outcome::Nothing,
        },
    }
}

pub fn punishment(
    guild: Snowflake,
    actor: Snowflake,
    target: Snowflake,
    outcome: Outcome,
    reason: Option<&str>,
    now: DateTime<Utc>,
) -> Option<Punishment> {
    let verb = match outcome {
        Outcome::Applied(_) => PunishmentType::Mute,
        Outcome::Ended => PunishmentType::Unmute,
        Outcome::Nothing => return None,
    };

    let punishment = Punishment::new(verb, guild, actor, target).reason(given(reason));

    Some(match outcome {
        Outcome::Applied(until) => punishment.duration(until - now),
        _ => punishment,
    })
}

fn given(reason: Option<&str>) -> Reason {
    Reason::new(reason.unwrap_or_default())
}

pub async fn observe(
    app: &App,
    http: impl CacheHttp,
    involved: Involved,
    before: Option<DateTime<Utc>>,
    after: Option<DateTime<Utc>>,
    reason: Option<&str>,
    now: DateTime<Utc>,
) -> Result<()> {
    let Involved {
        guild,
        actor,
        target,
        bot,
    } = involved;

    let outcome = read(before, after, reason, actor == bot, now);

    let Some(mut recorded) = punishment(guild, actor, target, outcome, reason, now) else {
        return Ok(());
    };

    store::supersede(&app.pool, &recorded).await?;
    store::insert(&app.pool, &mut recorded).await?;

    if outcome == Outcome::Ended
        && let Some(muted) = records::active(&app.pool, guild, target, PunishmentType::Mute).await?
    {
        store::mark_state(&app.pool, &muted.id, PunishmentState::Revoked).await?;
    }

    let actor_name = fetch::user(&http, UserId::new(actor))
        .await
        .ok()
        .map(|found| found.name);
    let target_name = fetch::user(&http, UserId::new(target))
        .await
        .ok()
        .map(|found| found.name);

    guildlog::post(
        app,
        http,
        guild,
        LogType::MemberModeration,
        &ui::log_entry(&recorded, actor_name.as_deref(), target_name.as_deref()),
        guildlog::Subject {
            target,
            moderator: Some(actor),
            action: Some(recorded.id.clone()),
        },
    )
    .await
    .map(|_| ())
}

pub async fn removed(
    app: &App,
    http: impl CacheHttp,
    involved: Involved,
    verb: PunishmentType,
    reason: Option<&str>,
) -> Result<()> {
    let Involved {
        guild,
        actor,
        target,
        bot,
    } = involved;

    if actor == bot || reason.is_some_and(|given| given.contains("Aegis Managed")) {
        return Ok(());
    }

    let mut recorded = Punishment::new(verb, guild, actor, target).reason(given(reason));

    store::supersede(&app.pool, &recorded).await?;
    store::insert(&app.pool, &mut recorded).await?;

    let actor_name = fetch::user(&http, UserId::new(actor))
        .await
        .ok()
        .map(|found| found.name);
    let target_name = fetch::user(&http, UserId::new(target))
        .await
        .ok()
        .map(|found| found.name);

    guildlog::post(
        app,
        http,
        guild,
        LogType::MemberModeration,
        &ui::log_entry(&recorded, actor_name.as_deref(), target_name.as_deref()),
        guildlog::Subject {
            target,
            moderator: Some(actor),
            action: Some(recorded.id.clone()),
        },
    )
    .await
    .map(|_| ())
}
