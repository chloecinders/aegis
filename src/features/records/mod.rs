pub mod amend;
pub mod commands;
pub mod controls;
pub mod store;
pub mod ui;

use serenity::all::{CacheHttp, EditMessage, UserId};
use sqlx::PgPool;

use crate::command::Response;
use crate::command::cx::Cx;
use crate::command::error::{Ctx, Result};
use crate::command::registry::Registry;
use crate::domain::action::Action;
use crate::features::{guildlog, references};
use crate::platform::discord::fetch;
use crate::platform::discord::interact::Router;
use crate::platform::ui::embed::Embed;
use crate::platform::ui::punishment;
use crate::platform::ui::reply;
use crate::register;

pub async fn answer(cx: &Cx, entry: Embed, replied: bool) -> Result<Response> {
    if !replied {
        return Ok(Response::embed(entry));
    }

    if let Err(failure) = cx
        .msg
        .delete(&cx.ctx.http, None)
        .await
        .ctx("clear amendment command")
    {
        cx.report(&failure);
    }

    Ok(Response::None)
}

pub async fn refreshed(pool: &PgPool, http: impl CacheHttp, action: &Action) -> Result<()> {
    let located = guildlog::store::locate(pool, action.guild, &action.id).await?;

    let Some((channel, message)) = located else {
        return Ok(());
    };

    let reference = references::store::load(pool, action.guild, &action.id).await?;
    let actor_name = fetch::user(&http, UserId::new(action.actor))
        .await
        .ok()
        .map(|found| found.name);
    let target_name = fetch::user(&http, UserId::new(action.target))
        .await
        .ok()
        .map(|found| found.name);
    let entry = punishment::log_entry(
        &action.to_punishment(),
        actor_name.as_deref(),
        target_name.as_deref(),
    );
    let controls = controls::attached(
        action.actor,
        &action.id,
        reference.as_ref(),
        action.note.as_ref(),
    );

    let rows = match controls.is_empty() {
        true => Vec::new(),
        false => vec![reply::row(&controls)],
    };

    channel
        .widen()
        .edit_message(
            http.http(),
            message,
            EditMessage::new()
                .embeds(vec![entry.build().into_owned()])
                .components(rows),
        )
        .await
        .ctx("refresh log entry")?;

    Ok(())
}

pub fn register(registry: &mut Registry) {
    register!(
        registry,
        commands::log::Log,
        commands::duration::SetDuration,
        commands::edit_ref::EditRef,
        commands::reason::SetReason,
        commands::note::SetNote,
    );
}

pub fn control(router: &mut Router) {
    controls::register(router);
}
