pub mod commands;
pub mod controls;
pub mod store;
pub mod ui;

use serenity::all::{
    Attachment, CacheHttp, ChannelId, GenericChannelId, Message, MessageId, Permissions,
};
use sqlx::PgPool;

use crate::command::args::ArgKind;
use crate::command::cx::Cx;
use crate::command::error::{Ctx, Error, Result};
use crate::command::registry::Registry;
use crate::command::value::{FromArgs, Snapshot};
use crate::domain::Snowflake;
use crate::domain::ids::ActionId;
use crate::platform::discord::fetch;
use crate::platform::discord::interact::Router;
use crate::platform::discord::permissions::Actor;
use crate::platform::s3;
use crate::platform::text::lexer::Token;
use crate::register;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Origin {
    Live,
    Archived,
}

impl Origin {
    pub fn as_str(&self) -> &'static str {
        match self {
            Origin::Live => "live",
            Origin::Archived => "archived",
        }
    }

    pub fn parse(raw: &str) -> Option<Self> {
        match raw {
            "live" => Some(Origin::Live),
            "archived" => Some(Origin::Archived),
            _ => None,
        }
    }

    pub fn jumpable(&self) -> bool {
        matches!(self, Origin::Live)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Reference {
    pub guild: Snowflake,
    pub channel: Snowflake,
    pub message: Snowflake,
}

pub fn from_link(raw: &str) -> Option<Reference> {
    let tail = raw.split_once("/channels/")?.1;
    let mut parts = tail.split('/');

    Some(Reference {
        guild: parts.next()?.parse().ok()?,
        channel: parts.next()?.parse().ok()?,
        message: parts.next()?.parse().ok()?,
    })
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Attached {
    pub content: bool,
    pub image: bool,
}

#[derive(Clone, Debug)]
pub struct Captured {
    pub origin: Origin,
    pub channel: Snowflake,
    pub message: Snowflake,
    pub author: Snowflake,
    pub content: Option<String>,
    pub image_url: Option<String>,
}

fn image(source: &Message) -> Option<&Attachment> {
    source.attachments.iter().find(|attachment| {
        attachment
            .content_type
            .as_deref()
            .is_some_and(|kind| kind.starts_with("image/"))
    })
}

impl Captured {
    pub fn of(source: &Message, origin: Origin) -> Self {
        Self {
            origin,
            channel: source.channel_id.get(),
            message: source.id.get(),
            author: source.author.id.get(),
            content: match source.content.is_empty() {
                true => None,
                false => Some(source.content.to_string()),
            },
            image_url: image(source).map(|attachment| attachment.url.to_string()),
        }
    }

    pub fn attached(source: &Message) -> Option<Self> {
        Some(Self {
            origin: Origin::Live,
            channel: source.channel_id.get(),
            message: source.id.get(),
            author: source.author.id.get(),
            content: None,
            image_url: Some(image(source)?.url.to_string()),
        })
    }

    pub fn has_content(&self) -> bool {
        self.content.is_some()
    }

    pub fn has_image(&self) -> bool {
        self.image_url.is_some()
    }
}

async fn readable(cx: &Cx, channel: Snowflake) -> Result<bool> {
    if channel == cx.channel_id().get() {
        return Ok(true);
    }

    let guild = cx.guild_id()?;
    let overwrites = fetch::overwrites(&cx.ctx, guild, GenericChannelId::new(channel)).await?;

    let snapshot = cx.guild().await?;
    let actor = cx.actor().await?;

    Ok(snapshot.allows(
        Actor {
            id: actor.user.id,
            roles: &actor.roles,
        },
        &overwrites.entries,
        Permissions::VIEW_CHANNEL | Permissions::READ_MESSAGE_HISTORY,
    ))
}

impl FromArgs for Reference {
    const KIND: ArgKind = ArgKind::Reference;

    async fn from_token(cx: &Cx, field: &'static str, token: &Token) -> Result<Self> {
        let invalid = || Error::invalid(cx.input(), field, ArgKind::Reference, token.span);
        let here = cx.guild_snowflake()?;

        let parsed = match from_link(&token.raw) {
            Some(parsed) => parsed,
            None => Reference {
                guild: here,
                channel: cx.channel_id().get(),
                message: token.raw.parse::<Snowflake>().map_err(|_| invalid())?,
            },
        };

        if parsed.guild != here {
            return Err(Error::new(cx.input())
                .title("referenced message not from this server")
                .with_span(token.span, "from another server"));
        }

        match readable(cx, parsed.channel).await? {
            true => Ok(parsed),
            false => Err(Error::new(cx.input())
                .title("cannot read this channel")
                .with_span(
                    token.span,
                    "messages in this channel are not readable by you",
                )),
        }
    }

    async fn from_message(cx: &Cx) -> Result<Option<Self>> {
        if image(&cx.msg).is_none() {
            return Ok(None);
        }

        Ok(Some(Reference {
            guild: cx.guild_snowflake()?,
            channel: cx.channel_id().get(),
            message: cx.msg.id.get(),
        }))
    }
}

impl Snapshot for Reference {
    fn snapshot(&self) -> serde_json::Value {
        serde_json::Value::from(self.message)
    }
}

pub async fn archive_image(cx: &Cx, guild: Snowflake, source: &str) -> Option<String> {
    let config = s3::settings(cx.app.config.s3.as_ref())?;
    let bytes = cx.app.http.bytes(source, 8 * 1024 * 1024).await.ok()?;
    let encoded = s3::to_webp(&bytes)?;
    let (key, url) = s3::placement(config, guild, &s3::token());

    match s3::store(config, &key, &encoded).await {
        true => Some(url),
        false => None,
    }
}

pub async fn capture(cx: &Cx, reference: Option<Reference>) -> Option<Captured> {
    let reference = reference?;

    let mut captured = match reference.message == cx.msg.id.get() {
        true => Captured::attached(&cx.msg)?,
        false => {
            let fetched = cx
                .ctx
                .http
                .get_message(reference.channel.into(), reference.message.into())
                .await
                .ok()?;

            Captured::of(&fetched, Origin::Live)
        }
    };

    if let Some(source) = captured.image_url.clone()
        && s3::settings(cx.app.config.s3.as_ref()).is_some()
        && let Ok(guild) = cx.guild_snowflake()
    {
        captured.image_url = archive_image(cx, guild, &source).await;
    }

    Some(captured)
}

pub async fn confirm(
    pool: &PgPool,
    http: impl CacheHttp,
    action: &ActionId,
    captured: &mut Captured,
) -> Result<()> {
    if !captured.origin.jumpable() {
        return Ok(());
    }

    let fetched = ChannelId::new(captured.channel)
        .widen()
        .message(http, MessageId::new(captured.message))
        .await
        .ctx("fetch referenced message");

    if !fetched.as_ref().is_err_and(Error::not_found) {
        return Ok(());
    }

    captured.origin = Origin::Archived;

    store::archive(pool, action).await
}

pub fn control(router: &mut Router) {
    controls::register(router);
}

pub fn register(registry: &mut Registry) {
    register!(registry, commands::view::ViewRef);
}
