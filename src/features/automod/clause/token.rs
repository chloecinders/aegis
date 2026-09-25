use chrono::Duration;
use serenity::all::Permissions;

use crate::command::error::Error;
use crate::domain::Snowflake;
use crate::platform::text::duration;
use crate::platform::text::lexer::{Span, Token, lex};

use super::Parsed;

pub struct Line {
    raw: String,
    start: usize,
    tokens: Vec<Token>,
}

impl Line {
    pub fn keyword(&self) -> &Token {
        &self.tokens[0]
    }

    pub fn rest(&self) -> &[Token] {
        &self.tokens[1..]
    }

    pub fn verbatim(&self) -> Option<(String, Span)> {
        let from = self.keyword().span.end() - self.start;
        let tail = self.raw.get(from..)?;
        let trimmed = tail.trim();

        if trimmed.is_empty() {
            return None;
        }

        let opener = trimmed.chars().next()?;
        let lead = from + tail.len() - tail.trim_start().len();
        let quoted =
            matches!(opener, '"' | '\'') && trimmed.len() >= 2 && trimmed.ends_with(opener);
        let body = match quoted {
            true => &trimmed[opener.len_utf8()..trimmed.len() - opener.len_utf8()],
            false => trimmed,
        };

        Some((
            body.to_string(),
            Span {
                start: self.start + lead + usize::from(quoted),
                len: body.len(),
                index: self.keyword().span.index + 1,
                quoted,
            },
        ))
    }
}

pub fn lines(block: &str, offset: usize) -> Vec<Line> {
    let mut at = 0;
    let mut out = Vec::new();

    for raw in block.split_inclusive('\n') {
        let start = at;

        at += raw.len();

        let tokens: Vec<Token> = lex(raw)
            .into_iter()
            .map(|mut token| {
                token.span.start += offset + start;
                token
            })
            .collect();

        if !tokens.is_empty() {
            out.push(Line {
                raw: raw.to_string(),
                start: offset + start,
                tokens,
            });
        }
    }

    out
}

pub fn window(tokens: &[Token]) -> Option<Duration> {
    match tokens {
        [amount] => duration::parse(&amount.raw),
        [amount, unit] => duration::words(&amount.raw, &unit.raw),
        _ => None,
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Mention {
    Channel,
    Role,
}

impl Mention {
    pub fn tag(&self, id: Snowflake) -> String {
        match self {
            Mention::Channel => format!("channel:{id}"),
            Mention::Role => format!("role:{id}"),
        }
    }
}

pub fn mention(raw: &str) -> Option<(Mention, Snowflake)> {
    if let Some(id) = raw.strip_prefix("role:") {
        return id.parse().ok().map(|id| (Mention::Role, id));
    }

    if let Some(id) = raw.strip_prefix("channel:") {
        return id.parse().ok().map(|id| (Mention::Channel, id));
    }

    let inner = raw.strip_prefix('<')?.strip_suffix('>')?;

    if let Some(id) = inner.strip_prefix("@&") {
        return id.parse().ok().map(|id| (Mention::Role, id));
    }

    if let Some(id) = inner.strip_prefix('#') {
        return id.parse().ok().map(|id| (Mention::Channel, id));
    }

    None
}

pub fn permission(raw: &str) -> Option<Permissions> {
    let name = raw.strip_prefix("permission:")?;

    Permissions::from_name(&name.replace('-', "_").to_uppercase())
}

pub fn channel(token: &Token) -> Parsed<Snowflake> {
    match mention(&token.raw) {
        Some((Mention::Channel, id)) => Ok(id),
        Some((Mention::Role, _)) => Err(Error::empty()
            .title("invalid rule clause")
            .with_span(token.span, "found role, expected channel")
            .with_span_help(token.span, "provide a channel", "channel:<id>")),
        None => token.raw.parse().map_err(|_| {
            Error::empty()
                .title("invalid rule clause")
                .with_span(token.span, "expected channel:<id>")
                .with_span_help(token.span, "provide a channel", "channel:<id>")
        }),
    }
}

pub fn count(token: &Token) -> Parsed<i64> {
    token.raw.parse().map_err(|_| {
        Error::empty()
            .title("invalid rule clause")
            .with_span(token.span, "expected whole number")
            .with_span_help(token.span, "provide a whole number", "5")
    })
}
