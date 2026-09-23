use serenity::all::{Member, VoiceState};
use serenity::async_trait;

use crate::command::error::Result;
use crate::domain::logtype::LogType;
use crate::features::guildlog::attribution::Attribution;
use crate::features::guildlog::member::{self, Snapshot};
use crate::features::guildlog::{self, Subject, attribution, voice};
use crate::features::punishments::store as punishments;
use crate::platform::discord::dispatch::{MemberCx, Observer, VoiceCx};

pub struct Watch;

pub fn snapshot(member: &Member) -> Snapshot {
    Snapshot {
        nick: member.nick.as_ref().map(ToString::to_string),
        roles: member.roles.iter().map(|role| role.get()).collect(),
        timeout: member.communication_disabled_until.map(|until| *until),
    }
}

async fn record(cx: &MemberCx) -> Result<()> {
    let (Some(before), Some(after)) = (&cx.previous, &cx.member) else {
        return Ok(());
    };

    if before.nick == after.nick
        && before.communication_disabled_until == after.communication_disabled_until
        && before.roles == after.roles
    {
        return Ok(());
    }

    let changed = member::diff(&snapshot(before), &snapshot(after));

    if changed.is_empty() {
        return Ok(());
    }

    let guild = cx.guild.get();
    let target = cx.user.id.get();
    let bot = cx.ctx.cache.current_user().id.get();

    let parts = changed.parts();

    let witnessed = cx.app.awaiting.claim(guild, target, &parts);

    let known = match cx.app.pending.claim_timeout(guild, target) {
        true => Attribution::Bot(bot),
        false => Attribution::Unknown,
    }
    .or(witnessed.actor);

    if matches!(known, Attribution::Bot(_))
        && changed.timeout.is_some()
        && changed.nick.is_none()
        && changed.gained.is_empty()
        && changed.lost.is_empty()
    {
        return Ok(());
    }

    let roles_only = changed.nick.is_none() && changed.timeout.is_none();

    if roles_only && known.actor() == Some(target) {
        return Ok(());
    }

    let actor_name = attribution::username(&cx.ctx, known, bot).await;

    let entry = member::entry(
        target,
        Some(&cx.user.name),
        &changed,
        known,
        actor_name.as_deref(),
        witnessed.reason.as_deref(),
        bot,
    );
    let Some(at) = guildlog::post(
        &cx.app,
        &cx.ctx,
        guild,
        LogType::MemberUpdate,
        &entry,
        Subject {
            target,
            moderator: known.actor(),
            action: None,
        },
    )
    .await?
    else {
        return Ok(());
    };

    let late = cx.app.awaiting.claim(guild, target, &parts);
    let arrived = late.actor.is_resolved() || late.reason.is_some();
    let known = known.or(late.actor);

    if roles_only && known.actor() == Some(target) {
        return guildlog::retract(&cx.app, &cx.ctx, at).await;
    }

    let reason = witnessed.reason.or(late.reason);

    let entry = match arrived {
        true => {
            let actor_name = attribution::username(&cx.ctx, known, bot).await;

            member::entry(
                target,
                Some(&cx.user.name),
                &changed,
                known,
                actor_name.as_deref(),
                reason.as_deref(),
                bot,
            )
        }
        false => entry,
    };

    if arrived {
        guildlog::store::attribute(&cx.app.pool, at.message.get(), known.actor()).await?;
        guildlog::rewrite(&cx.ctx, at, &entry).await?;
    }

    cx.app.awaiting.track(
        guild,
        target,
        &parts,
        at,
        &entry,
        known.is_resolved() && reason.is_some(),
    );

    Ok(())
}

async fn arrived(cx: &MemberCx) -> Result<()> {
    let guild = cx.guild.get();
    let target = cx.user.id.get();

    let history = punishments::record_count(&cx.app.pool, guild, target).await?;

    guildlog::post(
        &cx.app,
        &cx.ctx,
        guild,
        LogType::MemberJoinLeave,
        &member::joined(
            target,
            Some(&cx.user.name),
            *cx.user.id.created_at(),
            history,
        ),
        Subject {
            target,
            moderator: None,
            action: None,
        },
    )
    .await
    .map(|_| ())
}

async fn departed(cx: &MemberCx) -> Result<()> {
    let guild = cx.guild.get();
    let target = cx.user.id.get();

    guildlog::post(
        &cx.app,
        &cx.ctx,
        guild,
        LogType::MemberJoinLeave,
        &member::left(target, Some(&cx.user.name)),
        Subject {
            target,
            moderator: None,
            action: None,
        },
    )
    .await
    .map(|_| ())
}

async fn spoke(cx: &VoiceCx) -> Result<()> {
    let changed = voice::diff(cx.previous.as_ref().map(presence), presence(&cx.current));

    if changed.is_empty() {
        return Ok(());
    }

    let guild = cx.guild.get();
    let target = cx.user.get();

    let name = cx
        .current
        .member
        .as_ref()
        .map(|member| member.user.name.as_str());

    for entry in voice::entries(target, name, &changed) {
        guildlog::post(
            &cx.app,
            &cx.ctx,
            guild,
            LogType::VoiceActivity,
            &entry,
            Subject {
                target,
                moderator: None,
                action: None,
            },
        )
        .await?;
    }

    Ok(())
}

fn presence(state: &VoiceState) -> voice::Presence {
    voice::Presence {
        channel: state.channel_id.map(|channel| channel.get()),
        mute: state.mute(),
        deaf: state.deaf(),
    }
}

#[async_trait]
impl Observer for Watch {
    fn name(&self) -> &'static str {
        "guildlog"
    }

    async fn on_member_add(&self, cx: &MemberCx) {
        if cx.user.bot() {
            return;
        }

        if let Err(failure) = arrived(cx).await {
            cx.app
                .reporter
                .note("could not log a member joining", failure.to_string());
        }
    }

    async fn on_member_remove(&self, cx: &MemberCx) {
        if cx.user.bot() {
            return;
        }

        if let Err(failure) = departed(cx).await {
            cx.app
                .reporter
                .note("could not log a member leaving", failure.to_string());
        }
    }

    async fn on_voice_state(&self, cx: &VoiceCx) {
        if cx.bot {
            return;
        }

        if let Err(failure) = spoke(cx).await {
            cx.app
                .reporter
                .note("could not log a voice state change", failure.to_string());
        }
    }

    async fn on_member_update(&self, cx: &MemberCx) {
        if cx.user.bot() {
            return;
        }

        if let Err(failure) = record(cx).await {
            cx.app
                .reporter
                .note("could not log a member update", failure.to_string());
        }
    }
}
