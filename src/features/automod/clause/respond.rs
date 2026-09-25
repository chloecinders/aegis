use chrono::Duration;

use crate::command::error::Error;
use crate::domain::punishment::PunishmentType;
use crate::features::automod::rule::{Notify, Threshold};
use crate::platform::text::lexer::Span;

use super::Parsed;
use super::draft::Draft;
use super::token::{Line, Mention, channel, count, mention, permission, window};

impl Draft {
    pub fn only(&mut self, line: &Line) -> Parsed<()> {
        let rest = line.rest();

        if rest.is_empty() {
            return Err(Error::empty()
                .title("invalid rule clause")
                .with_span(line.keyword().span, "missing channel")
                .with_span_help(
                    line.keyword().span,
                    "provide a channel",
                    "only channel:<id>",
                ));
        }

        for token in rest {
            let id = channel(token)?;

            if !self.body.only.contains(&id) {
                self.body.only.push(id);
            }
        }

        Ok(())
    }

    pub fn ignore(&mut self, line: &Line) -> Parsed<()> {
        let rest = line.rest();

        if rest.is_empty() {
            return Err(Error::empty()
                .title("invalid rule clause")
                .with_span(line.keyword().span, "missing role, channel or permission")
                .with_span_help(line.keyword().span, "provide one", "ignore role:<id>"));
        }

        for token in rest {
            if token.raw.starts_with("permission:") {
                let flag = permission(&token.raw).ok_or_else(|| {
                    Error::empty()
                        .title("invalid rule clause")
                        .with_span(token.span, "no permission found")
                        .with_span_help(
                            token.span,
                            "provide a valid permission",
                            "permission:manage_messages",
                        )
                })?;

                self.body.ignore_permissions |= flag;

                continue;
            }

            let (kind, id) = mention(&token.raw).ok_or_else(|| {
                Error::empty()
                    .title("invalid rule clause")
                    .with_span(
                        token.span,
                        "expected role:<id>, channel:<id> or permission:<name>",
                    )
                    .with_span_help(token.span, "provide one", "role:<id>")
            })?;

            let into = match kind {
                Mention::Role => &mut self.body.ignore_roles,
                Mention::Channel => &mut self.body.ignore_channels,
            };

            if !into.contains(&id) {
                into.push(id);
            }
        }

        Ok(())
    }

    pub fn after(&mut self, line: &Line) -> Parsed<()> {
        let rest = line.rest();
        let here = rest.first().map_or(line.keyword().span, |token| token.span);

        if self.after.replace(here).is_some() {
            return Err(Error::empty()
                .title("invalid rule clause")
                .with_span(here, "duplicate after clause"));
        }

        let [times, joiner, written @ ..] = rest else {
            let sweep = rest.last().map_or(here, |last| Span {
                len: last.span.end() - here.start,
                ..here
            });
            let filled = match rest.first() {
                Some(count) => format!("{} in 10m", count.raw),
                None => "after 2 in 10m".to_string(),
            };

            return Err(Error::empty()
                .title("invalid rule clause")
                .with_span(here, "incomplete after clause")
                .with_span_help(sweep, "provide a valid threshold", filled));
        };

        if joiner.raw.to_lowercase() != "in" {
            let filled = match written.is_empty() {
                true => format!("in {}", joiner.raw),
                false => "in".to_string(),
            };

            return Err(Error::empty()
                .title("invalid rule clause")
                .with_span(joiner.span, "expected in")
                .with_span_help(joiner.span, "join the timeframe with in", filled));
        }

        let count = count(times)?;

        if count < 2 {
            return Err(Error::empty()
                .title("invalid rule clause")
                .with_span(times.span, "count below 2")
                .with_span_help(times.span, "provide 2 or more", "2"));
        }

        let at = written.first().map_or(joiner.span, |token| token.span);
        let counted = window(written).ok_or_else(|| {
            let error = Error::empty()
                .title("invalid rule clause")
                .with_span(at, "not a duration");

            match written.last() {
                Some(last) => error.with_span_help(
                    Span {
                        len: last.span.end() - at.start,
                        ..at
                    },
                    "provide a duration",
                    "10m",
                ),
                None => error,
            }
        })?;

        if counted <= Duration::zero() {
            let last = written.last().map_or(at, |token| token.span);

            return Err(Error::empty()
                .title("invalid rule clause")
                .with_span(at, "empty timeframe")
                .with_span_help(
                    Span {
                        len: last.end() - at.start,
                        ..at
                    },
                    "provide a timeframe",
                    "10m",
                ));
        }

        self.body.after = Some(Threshold {
            count: count as u32,
            window: counted,
        });

        Ok(())
    }

    pub fn then(&mut self, line: &Line) -> Parsed<()> {
        let rest = line.rest();
        let here = rest.first().map_or(line.keyword().span, |token| token.span);

        if self.action.replace(here).is_some() {
            return Err(Error::empty()
                .title("invalid rule clause")
                .with_span(here, "duplicate then clause"));
        }

        let [verb, tail @ ..] = rest else {
            return Err(Error::empty()
                .title("invalid rule clause")
                .with_span(here, "missing action")
                .with_span_help(here, "provide an action", "then ban 7d"));
        };

        if verb.raw.to_lowercase() == "delete" {
            self.body.outcome.delete = true;

            return match tail.is_empty() {
                true => Ok(()),
                false => Err(Error::empty()
                    .title("invalid rule clause")
                    .with_span(tail[0].span, "delete has no arguments")),
            };
        }

        let parsed = PunishmentType::parse(&verb.raw.to_lowercase())
            .filter(|verb| !matches!(verb, PunishmentType::Unban | PunishmentType::Unmute))
            .ok_or_else(|| {
                Error::empty()
                    .title("invalid rule clause")
                    .with_span(verb.span, "no action found")
                    .with_span_help(verb.span, "provide a valid action", "ban")
            })?;

        self.body.outcome.punishment_type = Some(parsed);

        if tail.is_empty() {
            return Ok(());
        }

        let at = tail.first().map_or(verb.span, |token| token.span);

        if !parsed.has_duration() {
            return Err(Error::empty()
                .title("invalid rule clause")
                .with_span(at, "only bans and mutes have durations"));
        }

        self.body.outcome.duration = window(tail).ok_or_else(|| {
            let last = tail.last().map_or(at, |token| token.span);

            Error::empty()
                .title("invalid rule clause")
                .with_span(at, "not a duration")
                .with_span_help(
                    Span {
                        len: last.end() - at.start,
                        ..at
                    },
                    "provide a duration",
                    "7d",
                )
        })?;

        Ok(())
    }

    pub fn clear(&mut self, line: &Line) -> Parsed<()> {
        let rest = line.rest();
        let here = rest.first().map_or(line.keyword().span, |token| token.span);

        let Some(amount) = rest.first() else {
            return Err(Error::empty()
                .title("invalid rule clause")
                .with_span(here, "missing days")
                .with_span_help(here, "provide a number of days", "clear 1"));
        };

        let bare = amount.raw.chars().all(|ch| ch.is_ascii_digit());
        let days = match window(rest) {
            Some(parsed) if !bare || rest.len() > 1 => parsed.num_days(),
            _ => count(amount)?,
        };

        if !(0..=7).contains(&days) {
            let filled = match days < 0 {
                true => "0",
                false => "7",
            };

            return Err(Error::empty()
                .title("invalid rule clause")
                .with_span(amount.span, "discord clears at most 7 days")
                .with_span_help(amount.span, "provide 0 to 7 days", filled));
        }

        self.body.outcome.clear_days = days as u8;

        Ok(())
    }

    pub fn notify(&mut self, line: &Line) -> Parsed<()> {
        let rest = line.rest();
        let here = rest.first().map_or(line.keyword().span, |token| token.span);
        let target = rest.first().ok_or_else(|| {
            Error::empty()
                .title("invalid rule clause")
                .with_span(here, "missing channel")
                .with_span_help(here, "provide a channel or none", "notify channel:<id>")
        })?;

        self.body.outcome.notify = match target.raw.eq_ignore_ascii_case("none") {
            true => Notify::None,
            false => Notify::Channel(channel(target)?),
        };

        Ok(())
    }

    pub fn reason(&mut self, line: &Line) -> Parsed<()> {
        let (text, span) = line.verbatim().ok_or_else(|| {
            Error::empty()
                .title("invalid rule clause")
                .with_span(line.keyword().span, "missing reason")
                .with_span_help(line.keyword().span, "provide a reason", "reason scam bot")
        })?;

        if self.reason.replace(span).is_some() {
            return Err(Error::empty()
                .title("invalid rule clause")
                .with_span(span, "duplicate reason clause"));
        }

        self.body.outcome.reason = Some(text);

        Ok(())
    }
}
