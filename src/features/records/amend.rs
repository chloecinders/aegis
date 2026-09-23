use chrono::{Duration, Utc};
use serde_json::Value;
use serenity::all::{Context, EditMember, EditMessage, GuildId, MessageId, UserId};

use crate::app::App;
use crate::command::cx::Cx;
use crate::command::edit::Change;
use crate::command::error::{Ctx, Error, Result};
use crate::domain::Snowflake;
use crate::domain::action::{Action, Amendment};
use crate::domain::punishment::PunishmentType;
use crate::domain::reason::{Note, Reason};
use crate::features::punishments::scheduled::{self, Kind};
use crate::features::records::{refreshed, store, ui};
use crate::platform::text::duration::phrase;
use crate::platform::ui::embed::Embed;
use crate::platform::ui::marks::Marks;
use crate::platform::ui::punishment;

async fn retime(app: &App, ctx: &Context, action: &Action, window: Duration) -> Result<()> {
    if action.verb != PunishmentType::Mute || !action.state.active() {
        return Ok(());
    }

    let until = action.to_punishment().duration(window).timeout_until();

    app.pending.expect_timeout(action.guild, action.target);

    GuildId::new(action.guild)
        .edit_member(
            &ctx.http,
            UserId::new(action.target),
            EditMember::new()
                .audit_log_reason(action.to_punishment().duration(window).audit_marker())
                .disable_communication_until(until.into()),
        )
        .await
        .map(|_| ())
        .ctx("retime mute")
}

pub fn rendered(action: &Action, change: &Change) -> Embed {
    match change.policy {
        Amendment::Duration => ui::amended(action, "duration", &window(action, &change.after)),
        Amendment::Note if change.after.is_null() => ui::cleared(action, "note"),
        Amendment::Note => ui::amended(action, "note", &shown(&change.after)),
        _ => ui::amended(action, "reason", &shown(&change.after)),
    }
}

fn shown(value: &Value) -> String {
    match value {
        Value::Null => String::new(),
        Value::String(text) => text.clone(),
        other => other.to_string(),
    }
}

fn window(action: &Action, value: &Value) -> String {
    let set = phrase(Duration::seconds(value.as_i64().unwrap_or_default()));
    let total = phrase(action.duration());

    match total == set {
        true => set,
        false => format!("{set} (total: {total})"),
    }
}

pub async fn write(
    app: &App,
    ctx: &Context,
    action: &Action,
    changes: &[Change],
) -> Result<Action> {
    let pool = &app.pool;
    let mut updated = action.clone();

    for change in changes {
        match change.policy {
            Amendment::Reason => {
                let reason = Reason::new(change.after.as_str().unwrap_or_default());

                store::set_reason(pool, action.guild, &action.id, &reason).await?;
                updated.reason = reason;
            }
            Amendment::Note => {
                let note = change.after.as_str().and_then(Note::new);

                store::set_note(pool, action.guild, &action.id, note.as_ref()).await?;
                updated.note = note;
            }
            Amendment::Duration => {
                let span = Duration::seconds(change.after.as_i64().unwrap_or_default());

                store::set_expiry(pool, action.guild, &action.id, span).await?;
                retime(app, ctx, &updated, span).await?;

                let lift = match updated.state.active().then_some(updated.verb) {
                    Some(PunishmentType::Ban) => Some(Kind::LiftBan),
                    Some(PunishmentType::Mute) => Some(Kind::LiftMute),
                    _ => None,
                };

                if let Some(kind) = lift {
                    scheduled::cancel(pool, &updated.id, kind).await?;

                    if !span.is_zero() {
                        scheduled::schedule(
                            pool,
                            kind,
                            &updated.id,
                            updated.guild,
                            updated.target,
                            Utc::now() + span,
                        )
                        .await?;
                    }

                    if updated.verb == PunishmentType::Mute {
                        let expiry = (!span.is_zero()).then(|| Utc::now() + span);

                        scheduled::cancel(pool, &updated.id, Kind::RefreshTimeout).await?;
                        scheduled::schedule(
                            pool,
                            Kind::RefreshTimeout,
                            &updated.id,
                            updated.guild,
                            updated.target,
                            scheduled::next_refresh(expiry),
                        )
                        .await?;
                    }
                }

                updated.expires_at = (!span.is_zero()).then(|| Utc::now() + span);
            }
            Amendment::Never => {
                return Err(Error::internal(
                    "an unamendable change reached the amendment handlers",
                ));
            }
        }
    }

    updated.updated_at = Utc::now();

    Ok(updated)
}

pub async fn inline(
    cx: &Cx,
    action: &Action,
    changes: &[Change],
    response: Option<Snowflake>,
) -> Result<Action> {
    let updated = write(&cx.app, &cx.ctx, action, changes).await?;

    for outcome in [
        refreshed(cx.pool(), &cx.ctx, &updated).await,
        renotify(cx, &updated).await,
        reword(cx, &updated, changes, response).await,
    ] {
        if let Err(failure) = outcome {
            cx.report(&failure);
        }
    }

    Ok(updated)
}

async fn renotify(cx: &Cx, action: &Action) -> Result<()> {
    let Some(at) = cx.app.notices.notice(cx.msg.id.get()) else {
        return Ok(());
    };

    let name = cx.guild_name().await;
    let notice = punishment::notice(&action.to_punishment(), &name);

    at.channel
        .edit_message(
            &cx.ctx.http,
            at.message,
            EditMessage::new().embeds(vec![notice.build()]),
        )
        .await
        .ctx("revise punishment notice")?;

    Ok(())
}

async fn reword(
    cx: &Cx,
    action: &Action,
    changes: &[Change],
    response: Option<Snowflake>,
) -> Result<()> {
    let Some(response) = response else {
        return Ok(());
    };

    let reworded = match cx.app.notices.reply(cx.msg.id.get()) {
        Some(marks) => Some(punishment::reply(
            &action.to_punishment(),
            Marks {
                edited: true,
                ..marks
            },
        )),
        None => changes.first().map(|change| rendered(action, change)),
    };

    let Some(embed) = reworded else {
        return Ok(());
    };

    cx.channel_id()
        .edit_message(
            &cx.ctx.http,
            MessageId::new(response),
            EditMessage::new().embeds(vec![embed.build()]),
        )
        .await
        .ctx("revise command response")?;

    Ok(())
}

pub async fn apply(cx: &Cx, action: &Action, changes: &[Change]) -> Result<Action> {
    let updated = write(&cx.app, &cx.ctx, action, changes).await?;

    refreshed(cx.pool(), &cx.ctx, &updated).await?;

    Ok(updated)
}
