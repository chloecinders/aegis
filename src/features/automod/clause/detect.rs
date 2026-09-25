use chrono::Duration;

use crate::command::error::Error;
use crate::features::automod::rule::{Cmp, Condition, Matcher, Measure, Source};
use crate::platform::text::lexer::{Span, Token};

use super::Parsed;
use super::draft::Draft;
use super::token::{Line, count, window};

impl Draft {
    pub fn on(&mut self, line: &Line) -> Parsed<()> {
        let rest = line.rest();

        if rest.is_empty() {
            return Err(Error::empty()
                .title("invalid rule clause")
                .with_span(line.keyword().span, "no source found")
                .with_span_help(line.keyword().span, "provide a valid source", "on content"));
        }

        for token in rest {
            let source = Source::parse(&token.raw.to_lowercase()).ok_or_else(|| {
                Error::empty()
                    .title("invalid rule clause")
                    .with_span(token.span, "no source found")
                    .with_span_help(token.span, "provide a valid source", "content")
            })?;

            if !self.body.sources.contains(&source) {
                self.body.sources.push(source);
            }
        }

        Ok(())
    }

    pub fn pattern(&mut self, line: &Line, excluding: bool) -> Parsed<()> {
        let (raw, span) = line.verbatim().ok_or_else(|| {
            let filled = match excluding {
                true => "never \"nitro giveaway rules\"",
                false => "match \"free nitro\"",
            };

            Error::empty()
                .title("invalid rule clause")
                .with_span(line.keyword().span, "missing pattern")
                .with_span_help(line.keyword().span, "provide a pattern", filled)
        })?;

        let matcher = Matcher::parse(&raw).map_err(|problem| {
            Error::empty()
                .title("invalid rule clause")
                .with_span(span, problem)
        })?;

        let into = match excluding {
            true => &mut self.body.nevers,
            false => &mut self.body.matches,
        };

        if into.len() >= 64 {
            return Err(Error::empty()
                .title("invalid rule clause")
                .with_span(span, "too many patterns"));
        }

        into.push(matcher);

        if !excluding {
            self.matching.get_or_insert(span);
        }

        Ok(())
    }

    pub fn when(&mut self, line: &Line) -> Parsed<()> {
        let rest = line.rest();
        let here = rest.first().map_or(line.keyword().span, |token| token.span);

        let [subject, middle, bound, ..] = rest else {
            let written = rest.last().map_or(here, |last| Span {
                len: last.span.end() - here.start,
                ..here
            });
            let filled = match rest.first() {
                Some(measure) => format!("{} > 5", measure.raw),
                None => "when mentions > 5".to_string(),
            };

            return Err(Error::empty()
                .title("invalid rule clause")
                .with_span(here, "incomplete when clause")
                .with_span_help(written, "provide a valid when clause", filled));
        };

        let measure = Measure::parse(&subject.raw.to_lowercase()).ok_or_else(|| {
            Error::empty()
                .title("invalid rule clause")
                .with_span(subject.span, "no measure found")
                .with_span_help(subject.span, "provide a valid measure", "mentions")
        })?;

        let condition = match measure {
            measure if measure.counts_record() => {
                let cmp = Cmp::parse(&middle.raw).ok_or_else(|| {
                    Error::empty()
                        .title("invalid rule clause")
                        .with_span(middle.span, "expected >, <, >= or <=")
                        .with_span_help(middle.span, "compare with an operator", ">=")
                })?;
                let count = count(bound)?;

                if count < 0 {
                    return Err(Error::empty()
                        .title("invalid rule clause")
                        .with_span(bound.span, "negative count")
                        .with_span_help(
                            bound.span,
                            "provide 0 or more",
                            bound.raw.trim_start_matches('-'),
                        ));
                }

                Condition {
                    measure,
                    cmp,
                    bound: count,
                    window: Self::counted(&rest[3..])?,
                }
            }
            Measure::AccountAge => {
                let cmp = match middle.raw.to_lowercase().as_str() {
                    "younger" => Cmp::Below,
                    "older" => Cmp::Above,
                    _ => {
                        let filled = match bound.raw.to_lowercase() == "than" {
                            true => "younger",
                            false => "younger than",
                        };

                        return Err(Error::empty()
                            .title("invalid rule clause")
                            .with_span(middle.span, "expected younger or older")
                            .with_span_help(middle.span, "provide younger or older", filled));
                    }
                };

                let amount = &rest[3..];

                if bound.raw.to_lowercase() != "than" {
                    let filled = match amount.is_empty() {
                        true => format!("than {}", bound.raw),
                        false => "than".to_string(),
                    };

                    return Err(Error::empty()
                        .title("invalid rule clause")
                        .with_span(bound.span, "expected than")
                        .with_span_help(bound.span, "join the age with than", filled));
                }

                let at = amount.first().map_or(bound.span, |token| token.span);
                let age = window(amount).ok_or_else(|| {
                    let error = Error::empty()
                        .title("invalid rule clause")
                        .with_span(at, "not a duration");

                    match amount.last() {
                        Some(last) => error.with_span_help(
                            Span {
                                len: last.span.end() - at.start,
                                ..at
                            },
                            "provide a duration",
                            "7d",
                        ),
                        None => error,
                    }
                })?;

                Condition {
                    measure,
                    cmp,
                    bound: age.num_seconds(),
                    window: None,
                }
            }
            _ => Condition {
                measure,
                cmp: Cmp::parse(&middle.raw).ok_or_else(|| {
                    Error::empty()
                        .title("invalid rule clause")
                        .with_span(middle.span, "expected >, <, >= or <=")
                        .with_span_help(middle.span, "compare with an operator", ">")
                })?,
                bound: count(bound)?,
                window: None,
            },
        };

        self.conditions.push((subject.span, measure));
        self.body.conditions.push(condition);

        Ok(())
    }

    fn counted(tail: &[Token]) -> Parsed<Option<Duration>> {
        let [joiner, written @ ..] = tail else {
            return Ok(None);
        };

        if joiner.raw.to_lowercase() != "in" {
            let filled = match written.is_empty() {
                true => format!("in {}", joiner.raw),
                false => "in".to_string(),
            };

            return Err(Error::empty()
                .title("invalid rule clause")
                .with_span(joiner.span, "expected in")
                .with_span_help(joiner.span, "join the window with in", filled));
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
                    "30d",
                ),
                None => error,
            }
        })?;

        if counted <= Duration::zero() {
            let last = written.last().map_or(at, |token| token.span);

            return Err(Error::empty()
                .title("invalid rule clause")
                .with_span(at, "empty window")
                .with_span_help(
                    Span {
                        len: last.end() - at.start,
                        ..at
                    },
                    "provide a window",
                    "30d",
                ));
        }

        Ok(Some(counted))
    }
}
