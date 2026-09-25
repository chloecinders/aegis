use crate::command::error::Error;
use crate::features::automod::rule::{Body, Measure, Source};
use crate::platform::text::lexer::Span;

use super::{Parsed, Part};

#[derive(Default)]
pub struct Draft {
    pub body: Body,
    pub action: Option<Span>,
    pub after: Option<Span>,
    pub reason: Option<Span>,
    pub matching: Option<Span>,
    pub conditions: Vec<(Span, Measure)>,
}

impl Draft {
    pub fn into_body(mut self, whole: Span, part: Part) -> Parsed<Body> {
        if part == Part::Response {
            return Ok(self.body);
        }

        let sources = self.body.sources().to_vec();

        if let Some(span) = self.matching
            && !sources.iter().any(Source::yields_text)
        {
            return Err(Error::empty()
                .title("invalid rule clause")
                .with_span(span, "source has no text"));
        }

        if let Some((span, _)) = self
            .conditions
            .iter()
            .find(|(_, measure)| !sources.iter().any(|source| measure.available_on(*source)))
        {
            return Err(Error::empty()
                .title("invalid rule clause")
                .with_span(*span, "measure not available on this source"));
        }

        if self.body.matches.is_empty() && self.body.conditions.is_empty() {
            return Err(Error::empty()
                .title("invalid rule clause")
                .with_span(whole, "missing match or when clause"));
        }

        if part == Part::Detection {
            return Ok(self.body);
        }

        if !self.body.outcome.acts() {
            self.body.outcome.delete = true;
        }

        Ok(self.body)
    }
}
