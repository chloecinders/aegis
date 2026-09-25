mod detect;
mod docs;
mod draft;
mod render;
mod respond;
mod token;

pub use docs::{CLAUSES, Clause};
pub use render::render;

use crate::command::error::Error;
use crate::features::automod::rule::Body;
use crate::platform::text::lexer::Span;

use draft::Draft;
use token::lines;

type Parsed<T> = Result<T, Error>;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Part {
    Whole,
    Detection,
    Response,
}

impl Part {
    pub fn allows(&self, keyword: &str) -> bool {
        match self {
            Part::Whole => true,
            Part::Detection => ["on", "match", "never", "when"].contains(&keyword),
            Part::Response => [
                "then", "delete", "clear", "reason", "after", "notify", "ignore", "only",
            ]
            .contains(&keyword),
        }
    }

    fn refusal(&self) -> &'static str {
        match self {
            Part::Whole => "no clause found",
            Part::Detection => "this block only takes detection clauses",
            Part::Response => "this block only takes response clauses",
        }
    }
}

pub fn summaries(part: Part) -> String {
    CLAUSES
        .iter()
        .filter(|clause| part.allows(clause.keyword))
        .map(|clause| format!("`{}` - {}", clause.keyword, clause.short))
        .collect::<Vec<String>>()
        .join("\n")
}

pub fn parse(source: &str, from: usize) -> Parsed<Body> {
    parse_as(source, from, Part::Whole)
}

pub fn parse_as(source: &str, from: usize, part: Part) -> Parsed<Body> {
    read(source, from, part).map_err(|failure| failure.against(source))
}

fn read(source: &str, offset: usize, part: Part) -> Parsed<Body> {
    let block = source.get(offset..).unwrap_or_default();
    let whole = Span {
        start: offset,
        len: block.trim_end().len(),
        index: 0,
        quoted: false,
    };
    let mut draft = Draft::default();

    for line in lines(block, offset) {
        let keyword = line.keyword().raw.to_lowercase();

        if !part.allows(&keyword) && CLAUSES.iter().any(|clause| clause.keyword == keyword) {
            return Err(Error::empty()
                .title("invalid rule clause")
                .with_span(line.keyword().span, part.refusal()));
        }

        match keyword.as_str() {
            "on" => draft.on(&line)?,
            "match" => draft.pattern(&line, false)?,
            "never" => draft.pattern(&line, true)?,
            "when" => draft.when(&line)?,
            "only" => draft.only(&line)?,
            "ignore" => draft.ignore(&line)?,
            "after" => draft.after(&line)?,
            "then" => draft.then(&line)?,
            "clear" => draft.clear(&line)?,
            "notify" => draft.notify(&line)?,
            "reason" => draft.reason(&line)?,
            "delete" => draft.body.outcome.delete = true,
            _ => {
                return Err(Error::empty()
                    .title("invalid rule clause")
                    .with_span(line.keyword().span, "no clause found"));
            }
        }
    }

    draft.into_body(whole, part)
}
