use std::collections::HashSet;
use std::sync::Arc;

use chrono::Utc;
use serenity::all::{EditMember, GuildId, Http, UserId, UserPagination};
use serenity::async_trait;
use tracing::info;

use crate::app::App;
use crate::command::error::{Ctx, Result};
use crate::domain::Snowflake;
use crate::domain::punishment::{PunishmentState, PunishmentType};
use crate::features::punishments::{scheduled, store};
use crate::features::records::store as records;
use crate::platform::discord::dispatch::{MemberCx, Observer};

pub struct Sync;

async fn reapply(cx: &MemberCx) -> Result<()> {
    let Some(muted) = records::active(
        &cx.app.pool,
        cx.guild.get(),
        cx.user.id.get(),
        PunishmentType::Mute,
    )
    .await?
    else {
        return Ok(());
    };

    if muted.expires_at.is_some_and(|expiry| expiry <= Utc::now()) {
        return Ok(());
    }

    let punishment = muted.to_punishment();
    let until = scheduled::next_refresh(muted.expires_at) + chrono::Duration::hours(1);

    cx.app.pending.expect_timeout(muted.guild, muted.target);

    GuildId::new(muted.guild)
        .edit_member(
            &cx.ctx.http,
            UserId::new(muted.target),
            EditMember::new()
                .audit_log_reason(punishment.audit_marker())
                .disable_communication_until(until.into()),
        )
        .await
        .map(|_| ())
        .ctx("reapply mute on rejoin")?;

    store::set_presence(&cx.app.pool, muted.guild, muted.target, true).await?;
    scheduled::schedule(
        &cx.app.pool,
        scheduled::Kind::RefreshTimeout,
        &muted.id,
        muted.guild,
        muted.target,
        scheduled::next_refresh(muted.expires_at),
    )
    .await
}

async fn stand_down(cx: &MemberCx) -> Result<()> {
    store::set_presence(&cx.app.pool, cx.guild.get(), cx.user.id.get(), false).await?;

    let Some(muted) = records::active(
        &cx.app.pool,
        cx.guild.get(),
        cx.user.id.get(),
        PunishmentType::Mute,
    )
    .await?
    else {
        return Ok(());
    };

    scheduled::cancel(&cx.app.pool, &muted.id, scheduled::Kind::RefreshTimeout).await
}

#[async_trait]
impl Observer for Sync {
    fn name(&self) -> &'static str {
        "punishments"
    }

    async fn on_member_add(&self, cx: &MemberCx) {
        if let Err(failure) = reapply(cx).await {
            cx.app.reporter.note(
                "could not reapply a punishment to a returning member",
                failure.to_string(),
            );
        }
    }

    async fn on_member_remove(&self, cx: &MemberCx) {
        if let Err(failure) = stand_down(cx).await {
            cx.app
                .reporter
                .note("could not stand a punishment down", failure.to_string());
        }
    }
}

async fn sweep(app: &Arc<App>, http: &Http, guild: Snowflake) -> Result<()> {
    let banned = records::all_active(&app.pool, guild, PunishmentType::Ban).await?;

    if banned.is_empty() {
        return Ok(());
    }

    let mut live: HashSet<u64> = HashSet::new();
    let mut after = None;

    loop {
        let page = GuildId::new(guild)
            .bans(http, after.map(UserPagination::After), None)
            .await
            .ctx("list guild bans")?;

        let reached_end = page.is_empty();

        after = page.last().map(|ban| ban.user.id);

        for ban in &page {
            live.insert(ban.user.id.get());
        }

        if reached_end || after.is_none() {
            break;
        }
    }

    for action in banned {
        if live.contains(&action.target) {
            continue;
        }

        store::mark_state(&app.pool, &action.id, PunishmentState::Revoked).await?;
        scheduled::cancel(&app.pool, &action.id, scheduled::Kind::LiftBan).await?;
    }

    Ok(())
}

pub async fn on_boot(app: Arc<App>, http: Arc<Http>) {
    let guilds = match store::guilds_with_active(&app.pool).await {
        Ok(guilds) => guilds,
        Err(failure) => {
            app.reporter.note(
                "could not sync punishments against Discord",
                failure.to_string(),
            );

            return;
        }
    };

    for guild in guilds {
        if let Err(failure) = sweep(&app, &http, guild).await {
            app.reporter.note(
                "could not sync punishments against Discord",
                failure.to_string(),
            );
        }
    }

    info!("synced punishments against Discord");
}
