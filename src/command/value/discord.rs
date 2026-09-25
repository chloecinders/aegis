use serenity::all::{
    Color, GenericChannelId, GuildChannel, Member, MessageType, RoleId, User, UserId,
};
use serenity::nonmax::NonMaxU16;

use crate::command::args::{ArgKind, Inferred};
use crate::command::cx::Cx;
use crate::command::error::{Error, Result};
use crate::command::value::{FromArgs, FromReply, Snapshot};
use crate::domain::Snowflake;
use crate::features::records::store;
use crate::platform::text::lexer::{Span, Token};

pub fn snowflake(raw: &str) -> Option<Snowflake> {
    if let Ok(id) = raw.parse::<Snowflake>() {
        return Some(id);
    }

    let inner = raw.strip_prefix('<')?.strip_suffix('>')?;

    inner
        .trim_start_matches(['@', '#', '&', '!'])
        .parse::<Snowflake>()
        .ok()
}

async fn inferred_target(
    cx: &Cx,
    field: &'static str,
    span: Span,
    kind: ArgKind,
) -> Result<Option<(Snowflake, Inferred)>> {
    let Some(reply) = cx.msg.referenced_message.as_deref() else {
        return Ok(None);
    };

    cx.guild_id()?;

    let logged = store::log_target(cx.pool(), reply.id.get()).await?;
    let target = logged.unwrap_or_else(|| reply.author.id.get());

    if target == cx.bot_id().get() {
        return Err(Error::new(cx.input())
            .title("no target in replied message")
            .with_span(span, format!("expected <{field}: {}>", kind.label()))
            .with_span_help(
                span,
                format!("provide a valid {}", kind.label()),
                kind.example(),
            ));
    }

    let source = match (
        logged.is_some() || reply.author.id == cx.bot_id(),
        reply.kind,
    ) {
        (true, _) => Inferred::Bot,
        (false, MessageType::AutoModAction) => Inferred::SystemMessage,
        _ => Inferred::Message,
    };

    Ok(Some((target, source)))
}

impl FromArgs for Member {
    const KIND: ArgKind = ArgKind::Member;

    async fn from_token(cx: &Cx, _field: &'static str, token: &Token) -> Result<Self> {
        let missing = || {
            Error::new(cx.input())
                .title("member not found")
                .with_span(token.span, "not found in this server")
        };

        if let Some(id) = snowflake(&token.raw) {
            return cx.member(UserId::new(id)).await.map_err(|_| missing());
        }

        let guild = cx.guild_id()?;
        let found = guild
            .search_members(&cx.ctx.http, &token.raw, NonMaxU16::new(1))
            .await
            .unwrap_or_default();

        found.into_iter().next().ok_or_else(missing)
    }
}

impl FromReply for Member {
    async fn from_reply(
        cx: &Cx,
        field: &'static str,
        span: Span,
    ) -> Result<Option<(Self, Inferred)>> {
        let Some((target, source)) = inferred_target(cx, field, span, ArgKind::Member).await?
        else {
            return Ok(None);
        };

        let member = cx.member(UserId::new(target)).await.map_err(|_| {
            Error::new(cx.input())
                .title("member not found")
                .with_span(span, "not found in this server")
                .with_span_help(
                    span,
                    format!("provide a valid {}", ArgKind::Member.label()),
                    ArgKind::Member.example(),
                )
        })?;

        Ok(Some((member, source)))
    }
}

impl Snapshot for Member {
    fn snapshot(&self) -> serde_json::Value {
        serde_json::Value::from(self.user.id.get())
    }
}

impl FromArgs for User {
    const KIND: ArgKind = ArgKind::User;

    async fn from_token(cx: &Cx, _field: &'static str, token: &Token) -> Result<Self> {
        let missing = || {
            Error::new(cx.input())
                .title("user not found")
                .with_span(token.span, "unknown name or id")
        };

        if let Some(id) = snowflake(&token.raw) {
            return cx.user(UserId::new(id)).await.map_err(|_| missing());
        }

        let guild = cx.guild_id()?;

        cx.ctx
            .cache
            .guild(guild)
            .and_then(|cached| {
                cached
                    .members
                    .iter()
                    .find(|member| member.user.name == token.raw)
                    .map(|member| member.user.clone())
            })
            .ok_or_else(missing)
    }
}

impl FromReply for User {
    async fn from_reply(
        cx: &Cx,
        field: &'static str,
        span: Span,
    ) -> Result<Option<(Self, Inferred)>> {
        let Some((target, source)) = inferred_target(cx, field, span, ArgKind::User).await? else {
            return Ok(None);
        };

        let user = cx.user(UserId::new(target)).await.map_err(|_| {
            Error::new(cx.input())
                .title("user not found")
                .with_span(span, "no longer exists")
                .with_span_help(
                    span,
                    format!("provide a valid {}", ArgKind::User.label()),
                    ArgKind::User.example(),
                )
        })?;

        Ok(Some((user, source)))
    }
}

impl Snapshot for User {
    fn snapshot(&self) -> serde_json::Value {
        serde_json::Value::from(self.id.get())
    }
}

impl FromArgs for GuildChannel {
    const KIND: ArgKind = ArgKind::Channel;

    async fn from_token(cx: &Cx, _field: &'static str, token: &Token) -> Result<Self> {
        let missing = || {
            Error::new(cx.input())
                .title("channel not found")
                .with_span(token.span, "not found in this server")
        };
        let guild = cx.guild_id()?;

        if let Some(id) = snowflake(&token.raw) {
            return crate::platform::discord::fetch::channel(
                &cx.ctx,
                guild,
                serenity::all::ChannelId::new(id),
            )
            .await
            .map_err(|_| missing());
        }

        let wanted = token.raw.trim_start_matches('#');

        cx.ctx
            .cache
            .guild(guild)
            .and_then(|cached| {
                cached
                    .channels
                    .iter()
                    .find(|channel| channel.base.name == wanted)
                    .cloned()
            })
            .ok_or_else(missing)
    }
}

impl Snapshot for GuildChannel {
    fn snapshot(&self) -> serde_json::Value {
        serde_json::Value::from(self.id.get())
    }
}

impl FromArgs for GenericChannelId {
    const KIND: ArgKind = ArgKind::Channel;

    async fn from_token(cx: &Cx, field: &'static str, token: &Token) -> Result<Self> {
        snowflake(&token.raw)
            .map(GenericChannelId::new)
            .ok_or_else(|| Error::invalid(cx.input(), field, ArgKind::Channel, token.span))
    }
}

impl Snapshot for GenericChannelId {
    fn snapshot(&self) -> serde_json::Value {
        serde_json::Value::from(self.get())
    }
}

impl FromArgs for RoleId {
    const KIND: ArgKind = ArgKind::Role;

    async fn from_token(cx: &Cx, _field: &'static str, token: &Token) -> Result<Self> {
        let missing = || {
            Error::new(cx.input())
                .title("role not found")
                .with_span(token.span, "not found in this server")
        };

        if let Some(id) = snowflake(&token.raw) {
            return Ok(RoleId::new(id));
        }

        let guild = cx.guild_id()?;
        let wanted = token.raw.trim_start_matches('@');

        cx.ctx
            .cache
            .guild(guild)
            .and_then(|cached| {
                cached
                    .roles
                    .iter()
                    .find(|role| role.name == wanted)
                    .map(|role| role.id)
            })
            .ok_or_else(missing)
    }
}

impl Snapshot for RoleId {
    fn snapshot(&self) -> serde_json::Value {
        serde_json::Value::from(self.get())
    }
}

impl FromArgs for Color {
    const KIND: ArgKind = ArgKind::Color;

    async fn from_token(cx: &Cx, field: &'static str, token: &Token) -> Result<Self> {
        let digits = token.raw.trim_start_matches('#');

        if digits.len() != 6 {
            return Err(Error::invalid(
                cx.input(),
                field,
                ArgKind::Color,
                token.span,
            ));
        }

        u32::from_str_radix(digits, 16)
            .map(Color::new)
            .map_err(|_| Error::invalid(cx.input(), field, ArgKind::Color, token.span))
    }
}

impl Snapshot for Color {
    fn snapshot(&self) -> serde_json::Value {
        serde_json::Value::from(self.0)
    }
}
