pub mod amend;
pub mod attribution;
pub mod audit;
pub mod member;
pub mod poller;
pub mod render;
pub mod store;
pub mod voice;
pub mod watch;

use std::sync::Arc;

use serenity::all::{CacheHttp, ChannelId, CreateAttachment, EditMessage, MessageId};

use crate::app::App;
use crate::command::cx::Cx;
use crate::command::error::{Ctx, Result};
use crate::domain::Snowflake;
use crate::domain::ids::ActionId;
use crate::domain::logtype::LogType;
use crate::features::guildlog::attribution::Attribution;
use crate::platform::discord::dispatch::Dispatch;
use crate::platform::ui::embed::Embed;
use crate::platform::ui::reply::{self, Button};

pub fn observe(dispatch: &mut Dispatch) {
    dispatch.add(Arc::new(watch::Watch));
}

pub struct Subject {
    pub target: Snowflake,
    pub moderator: Option<Snowflake>,
    pub action: Option<ActionId>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Posted {
    pub channel: ChannelId,
    pub message: MessageId,
}

pub async fn emit(
    cx: &Cx,
    kind: LogType,
    embed: &Embed,
    subject: Subject,
    controls: &[Button],
) -> Result<Option<Posted>> {
    attaching(
        &cx.app,
        &cx.ctx,
        cx.guild_snowflake()?,
        kind,
        embed,
        subject,
        None,
        controls,
    )
    .await
}

pub async fn post(
    app: &App,
    http: impl CacheHttp,
    guild: Snowflake,
    kind: LogType,
    embed: &Embed,
    subject: Subject,
) -> Result<Option<Posted>> {
    attaching(app, http, guild, kind, embed, subject, None, &[]).await
}

pub async fn attaching(
    app: &App,
    http: impl CacheHttp,
    guild: Snowflake,
    kind: LogType,
    embed: &Embed,
    subject: Subject,
    file: Option<CreateAttachment<'static>>,
    controls: &[Button],
) -> Result<Option<Posted>> {
    let pool = &app.pool;
    let Some(channel) = app.settings.channel_for(pool, guild, kind).await? else {
        return Ok(None);
    };

    let mut entry = reply::plain(embed).components(
        controls
            .chunks(5)
            .take(5)
            .map(reply::row)
            .collect::<Vec<_>>(),
    );

    if let Some(file) = file {
        entry = entry.add_file(file);
    }

    let posted = channel
        .widen()
        .send_message(http.http(), entry)
        .await
        .ctx("send log entry")?;

    store::remember(
        pool,
        &store::Entry {
            message: posted.id,
            channel,
            guild,
            target: subject.target,
            moderator: subject.moderator,
            action: subject.action,
        },
    )
    .await?;

    Ok(Some(Posted {
        channel,
        message: posted.id,
    }))
}

pub async fn rewrite(http: impl CacheHttp, at: Posted, embed: &Embed) -> Result<()> {
    at.channel
        .widen()
        .edit_message(
            http.http(),
            at.message,
            EditMessage::new().embeds(vec![embed.build()]),
        )
        .await
        .ctx("amend log entry")?;

    Ok(())
}

pub async fn retract(app: &App, http: impl CacheHttp, at: Posted) -> Result<()> {
    at.channel
        .widen()
        .delete_message(http.http(), at.message, None)
        .await
        .ctx("retract log entry")?;

    store::forget(&app.pool, at.message.get()).await
}

pub async fn resolve(
    app: &App,
    http: impl CacheHttp,
    guild: Snowflake,
    target: Snowflake,
    channel: Snowflake,
    known: Attribution,
) -> Attribution {
    if !known.wants_poll() {
        return known;
    }

    let page = app.audit.recent(http, guild).await;
    let found = app
        .attributed
        .claim(guild, target, channel, &page, chrono::Utc::now());

    if !found.is_resolved() {
        app.audit.forget(guild);
    }

    known.or(found)
}
