use std::sync::Arc;
use std::time::Duration as Wait;

use serenity::all::{EditMember, GuildId, Http, UserId};
use tokio::time::sleep;
use tracing::warn;

use crate::app::App;
use crate::command::error::{Ctx, Result};
use crate::domain::ids::ActionId;
use crate::domain::punishment::PunishmentState;
use crate::features::punishments::scheduled::{self, Due, Kind};
use crate::features::records::store as records;

async fn refresh(app: &App, http: &Http, due: &Due, action: &ActionId) -> Result<()> {
    use crate::command::error::Ctx;

    let Some(record) = records::load(&app.pool, due.guild, action).await? else {
        return Ok(());
    };

    if !record.state.active() {
        return Ok(());
    }

    let until = scheduled::next_refresh(record.expires_at) + chrono::Duration::hours(1);

    GuildId::new(due.guild)
        .edit_member(
            http,
            UserId::new(record.target),
            EditMember::new()
                .audit_log_reason(format!("Aegis Managed: refreshing mute `{action}`"))
                .disable_communication_until(until.into()),
        )
        .await
        .map(|_| ())
        .ctx("refresh timeout")?;

    scheduled::finish(&app.pool, due.id).await?;
    scheduled::schedule(
        &app.pool,
        Kind::RefreshTimeout,
        action,
        due.guild,
        record.target,
        scheduled::next_refresh(record.expires_at),
    )
    .await
}

async fn perform(app: &App, http: &Http, due: &Due) -> Result<()> {
    let guild = GuildId::new(due.guild);
    let Some(user) = due.user.map(UserId::new) else {
        return Ok(());
    };

    if matches!(due.kind, Kind::LiftMute | Kind::RefreshTimeout) {
        app.pending.expect_timeout(due.guild, user.get());
    }

    match due.kind {
        Kind::LiftBan => guild.unban(http, user, None).await.ctx("lift ban"),
        Kind::LiftMute => guild
            .edit_member(
                http,
                user,
                EditMember::new()
                    .audit_log_reason("mute has reached expiry")
                    .enable_communication(),
            )
            .await
            .map(|_| ())
            .ctx("lift mute"),
        Kind::RefreshTimeout => match due.action.as_ref() {
            Some(action) => refresh(app, http, due, action).await,
            None => Ok(()),
        },
    }
}

async fn record(app: &App, due: &Due, outcome: Result<()>) {
    let closing = match due.kind {
        Kind::RefreshTimeout => PunishmentState::Active,
        _ => PunishmentState::Ended,
    };

    let failure = match outcome {
        Ok(()) => {
            let _ = scheduled::mark_state(&app.pool, due, closing).await;
            let _ = scheduled::finish(&app.pool, due.id).await;

            return;
        }
        Err(failure) => failure,
    };

    if failure.not_found() {
        let _ = scheduled::mark_state(&app.pool, due, PunishmentState::Revoked).await;
        let _ = scheduled::finish(&app.pool, due.id).await;

        return;
    }

    app.reporter
        .note("could not carry out due work", failure.to_string());

    let recorded = match due.attempts >= 6 {
        true => scheduled::abandon(&app.pool, due, &failure.to_string()).await,
        false => scheduled::defer(&app.pool, due, &failure.to_string()).await,
    };

    if let Err(err) = recorded {
        warn!("could not record the outcome of due work; err = {err:?}");
    }
}

pub async fn drain(app: &App, http: &Http) {
    let claimed = match scheduled::claim(&app.pool).await {
        Ok(claimed) => claimed,
        Err(failure) => {
            app.reporter
                .note("could not claim due work", failure.to_string());

            return;
        }
    };

    for due in claimed {
        let outcome = perform(app, http, &due).await;

        record(app, &due, outcome).await;
    }
}

pub async fn supervise(app: Arc<App>, http: Arc<Http>) {
    sleep(Wait::from_millis(u64::from(std::process::id()) % 2500)).await;

    loop {
        drain(&app, &http).await;
        sleep(Wait::from_secs(60)).await;
    }
}
