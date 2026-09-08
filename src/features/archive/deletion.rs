use serenity::all::{ChannelId, GuildId, MessageId};

use crate::app::App;
use crate::command::error::Result;
use crate::domain::Snowflake;
use crate::domain::logtype::LogType;
use crate::features::archive::{reply_line, store};

#[cfg(feature = "web")]
use crate::features::archive::transcript::{self, Request};
use crate::features::guildlog::{self, attribution, attribution::Attribution};
use crate::platform::discord::dispatch::BulkDeletionCx;
#[cfg(feature = "web")]
use crate::platform::discord::fetch;
use crate::platform::discord::partial::PartialMessage;
use crate::platform::ui::embed::{Embed, channel_mention, mention};
use crate::platform::ui::tone::Tone;

pub fn entry(
    message: &PartialMessage,
    actor: Attribution,
    actor_name: Option<&str>,
    bot: Snowflake,
) -> Embed {
    let embed = Embed::new("MESSAGE DELETED")
        .subtitle(format!("ID: `{}`", message.id))
        .subtitle(format!(
            "Author: {}",
            mention(message.author.id, Some(&message.author.name))
        ))
        .subtitle(format!("Channel: <#{}>", message.channel_id))
        .maybe_subtitle(actor.line(bot, actor_name))
        .maybe_lead(reply_line(message))
        .tone(Tone::Danger);

    match message.content.is_empty() {
        true => embed,
        false => embed.quote(message.content.clone()),
    }
}

pub fn bulk_entry(
    channel: Snowflake,
    removed: usize,
    actor: Attribution,
    actor_name: Option<&str>,
    bot: Snowflake,
    transcript: Option<&str>,
) -> Embed {
    Embed::new("MESSAGES DELETED")
        .subtitle(format!("Channel: {}", channel_mention(channel)))
        .subtitle(format!("Removed: {removed}"))
        .maybe_subtitle(actor.line(bot, actor_name))
        .maybe_footnote(transcript.map(|link| format!("[View transcript]({link})")))
        .tone(Tone::Danger)
}

pub async fn record(
    app: &App,
    ctx: &serenity::all::Context,
    guild: GuildId,
    channel: ChannelId,
    message: MessageId,
) -> Result<()> {
    let Some(cached) = app.recent.take(channel.get(), message.get()) else {
        return Ok(());
    };

    if app.pending.claim_silence(channel.get(), message.get()) {
        return Ok(());
    }

    if cached.content.is_empty() && cached.attachments.is_empty() {
        return Ok(());
    }

    let bot = ctx.cache.current_user().id.get();
    let guild = guild.get() as Snowflake;
    let known = match app.pending.claim_deletion(channel.get(), message.get()) {
        true => Attribution::Bot(bot),
        false => Attribution::Unknown,
    };

    let actor = attribution::username(ctx, known, bot).await;

    let Some(at) = guildlog::post(
        app,
        ctx,
        guild,
        LogType::MessageUpdate,
        &entry(&cached, known, actor.as_deref(), bot),
        guildlog::Subject {
            target: cached.author.id,
            moderator: known.actor(),
            action: None,
        },
    )
    .await?
    else {
        return Ok(());
    };

    let key = (guild, cached.author.id, channel.get());
    let resolved = guildlog::resolve(app, ctx, guild, cached.author.id, channel.get(), known).await;

    if resolved == known {
        app.awaiting.expect(
            key,
            at,
            &entry(&cached, known, actor.as_deref(), bot),
            known,
        );

        return Ok(());
    }

    let actor = attribution::username(ctx, resolved, bot).await;

    app.awaiting.forget(&key);
    guildlog::store::attribute(&app.pool, at.message.get(), resolved.actor()).await?;
    guildlog::rewrite(ctx, at, &entry(&cached, resolved, actor.as_deref(), bot)).await
}

pub async fn bulk(cx: &BulkDeletionCx, guild: GuildId) -> Result<()> {
    let channel = cx.channel.get();
    let ids: Vec<Snowflake> = cx.messages.iter().map(|id| id.get()).collect();
    let mut purged = false;

    for id in &ids {
        purged |= cx.app.pending.claim_deletion(channel, *id);
        cx.app.recent.take(channel, *id);
    }

    let guild = guild.get() as Snowflake;

    store::removed_many(&cx.app.pool, guild, &ids).await?;

    if purged {
        return Ok(());
    }

    let bot = cx.ctx.cache.current_user().id.get();
    let known = cx.app.awaiting.claim_bulk(&(guild, channel));
    let actor = attribution::username(&cx.ctx, known, bot).await;
    let removed = ids.len();
    let link = preserve_messages(cx, guild, ids).await;
    let entry = bulk_entry(
        channel,
        removed,
        known,
        actor.as_deref(),
        bot,
        link.as_deref(),
    );

    let Some(at) = guildlog::post(
        &cx.app,
        &cx.ctx,
        guild,
        LogType::MessageUpdate,
        &entry,
        guildlog::Subject {
            target: channel,
            moderator: known.actor(),
            action: None,
        },
    )
    .await?
    else {
        return Ok(());
    };

    cx.app
        .awaiting
        .expect_bulk((guild, channel), at, &entry, known);

    Ok(())
}

pub async fn channel(
    app: &App,
    ctx: &serenity::all::Context,
    guild: GuildId,
    channel: Snowflake,
    name: Option<String>,
    actor: Snowflake,
    bot: Snowflake,
) {
    let guild = guild.get() as Snowflake;
    let link = preserve(app, guild, channel, name.clone()).await;
    let known = Attribution::Gateway(actor);
    let actor_name = attribution::username(ctx, known, bot).await;

    let entry = guildlog::render::channel_deleted(
        channel,
        name.as_deref(),
        known,
        actor_name.as_deref(),
        bot,
        link.as_deref(),
    );

    if let Err(failure) = guildlog::post(
        app,
        ctx,
        guild,
        LogType::Channels,
        &entry,
        guildlog::Subject {
            target: channel,
            moderator: Some(actor),
            action: None,
        },
    )
    .await
    {
        app.reporter.record(&failure, Default::default());
    }
}

#[cfg(feature = "web")]
async fn preserve(
    app: &App,
    guild: Snowflake,
    channel: Snowflake,
    name: Option<String>,
) -> Option<String> {
    let asked = Request::channel(guild, channel, name, String::from("Aegis"));

    match transcript::store::build(&app.pool, &asked).await {
        Ok(built) => {
            built.and_then(|id| transcript::url(app.config.web_url.as_deref(), guild, &id))
        }
        Err(failure) => {
            app.reporter.record(&failure, Default::default());

            None
        }
    }
}

#[cfg(not(feature = "web"))]
async fn preserve(
    _app: &App,
    _guild: Snowflake,
    _channel: Snowflake,
    _name: Option<String>,
) -> Option<String> {
    None
}

#[cfg(feature = "web")]
async fn preserve_messages(
    cx: &BulkDeletionCx,
    guild: Snowflake,
    ids: Vec<Snowflake>,
) -> Option<String> {
    let moderator_name = fetch::guild_name(&cx.ctx, GuildId::new(guild)).await;
    let asked = Request::selection(guild, ids, moderator_name);

    match transcript::store::build(&cx.app.pool, &asked).await {
        Ok(built) => {
            built.and_then(|id| transcript::url(cx.app.config.web_url.as_deref(), guild, &id))
        }
        Err(failure) => {
            cx.app.reporter.record(&failure, Default::default());

            None
        }
    }
}

#[cfg(not(feature = "web"))]
async fn preserve_messages(
    _cx: &BulkDeletionCx,
    _guild: Snowflake,
    _ids: Vec<Snowflake>,
) -> Option<String> {
    None
}
