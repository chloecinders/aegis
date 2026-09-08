use std::fmt::{self, Display};

use crate::command::args::ArgKind;
use crate::platform::text::lexer::Span;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Clone, Copy, Debug)]
pub enum Mark {
    All,
    At(Span),
}

#[derive(Clone, Debug)]
pub struct Label {
    pub mark: Mark,
    pub text: String,
}

#[derive(Clone, Debug)]
pub struct Help {
    pub span: Span,
    pub text: String,
    pub with: String,
}

#[derive(Debug)]
pub enum Cause {
    Discord {
        op: &'static str,
        source: Box<serenity::Error>,
    },
    Store {
        op: &'static str,
        source: Box<sqlx::Error>,
    },
    Internal {
        context: &'static str,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Audience {
    User,
    Operator,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Severity {
    Expected,
    Degraded,
    Bug,
}

#[derive(Debug, Default)]
struct Diagnostic {
    source: Option<String>,
    title: String,
    label: Option<Label>,
    help: Option<Help>,
    hint: Option<String>,
    cause: Option<Cause>,
}

#[derive(Debug, Default)]
pub struct Error(Box<Diagnostic>);

impl Error {
    pub fn new(source: impl Into<String>) -> Self {
        let mut error = Self::bare();

        error.0.source = Some(source.into());
        error
    }

    pub fn bare() -> Self {
        Self::default()
    }

    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.0.title = title.into();
        self
    }

    pub fn with_all(mut self, text: impl Into<String>) -> Self {
        self.0.label = Some(Label {
            mark: Mark::All,
            text: text.into(),
        });

        self
    }

    pub fn with_span(mut self, span: Span, text: impl Into<String>) -> Self {
        self.0.label = Some(Label {
            mark: Mark::At(span),
            text: text.into(),
        });

        self
    }

    pub fn with_span_help(
        mut self,
        span: Span,
        text: impl Into<String>,
        with: impl Into<String>,
    ) -> Self {
        self.0.help = Some(Help {
            span,
            text: text.into(),
            with: with.into(),
        });

        self
    }

    pub fn with_hint(mut self, hint: impl Into<String>) -> Self {
        self.0.hint = Some(hint.into());
        self
    }

    pub fn against(mut self, source: impl Into<String>) -> Self {
        self.0.source.get_or_insert_with(|| source.into());
        self
    }

    fn caused(mut self, cause: Cause) -> Self {
        self.0.cause = Some(cause);
        self
    }

    pub fn internal(context: &'static str) -> Self {
        Error::bare()
            .title("internal error")
            .caused(Cause::Internal { context })
    }

    pub fn missing(source: impl Into<String>, field: &str, kind: ArgKind, span: Span) -> Self {
        Error::new(source)
            .title("missing argument")
            .with_span(span, format!("expected <{field}: {}>", kind.label()))
            .with_span_help(
                span,
                format!("provide a valid {}", kind.label()),
                kind.example(),
            )
    }

    pub fn invalid(source: impl Into<String>, field: &str, kind: ArgKind, span: Span) -> Self {
        Error::new(source)
            .title("invalid argument")
            .with_span(span, format!("expected <{field}: {}>", kind.label()))
            .with_span_help(
                span,
                format!("provide a valid {}", kind.label()),
                kind.example(),
            )
    }

    pub fn headline(&self) -> &str {
        &self.0.title
    }

    pub fn source(&self) -> Option<&str> {
        self.0.source.as_deref()
    }

    pub fn label(&self) -> Option<&Label> {
        self.0.label.as_ref()
    }

    pub fn help(&self) -> Option<&Help> {
        self.0.help.as_ref()
    }

    pub fn hint(&self) -> Option<&str> {
        self.0.hint.as_deref()
    }

    pub fn cause(&self) -> Option<&Cause> {
        self.0.cause.as_ref()
    }

    pub fn span(&self) -> Option<Span> {
        match self.0.label.as_ref()?.mark {
            Mark::At(span) => Some(span),
            Mark::All => None,
        }
    }

    pub fn audience(&self) -> Audience {
        match self.0.cause {
            None => Audience::User,
            Some(_) => Audience::Operator,
        }
    }

    pub fn severity(&self) -> Severity {
        match &self.0.cause {
            None => Severity::Expected,
            Some(Cause::Discord { .. } | Cause::Store { .. }) => Severity::Degraded,
            Some(Cause::Internal { .. }) => Severity::Bug,
        }
    }

    pub fn not_found(&self) -> bool {
        let Some(Cause::Discord { source, .. }) = &self.0.cause else {
            return false;
        };

        matches!(
            source.as_ref(),
            serenity::Error::Http(serenity::all::HttpError::UnsuccessfulRequest(response))
                if response.status_code.as_u16() == 404
        )
    }

    pub fn detail(&self) -> Option<String> {
        match &self.0.cause {
            Some(Cause::Discord { op, source }) => {
                Some(format!("{op}: {}", chain(source.as_ref())))
            }
            Some(Cause::Store { op, source }) => Some(format!("{op}: {source}")),
            Some(Cause::Internal { context }) => Some(String::from(*context)),
            None => None,
        }
    }
}

fn chain(source: &dyn std::error::Error) -> String {
    let mut trail = vec![source.to_string()];
    let mut next = source.source();

    while let Some(inner) = next {
        let text = inner.to_string();

        if trail.last() != Some(&text) {
            trail.push(text);
        }

        next = inner.source();
    }

    trail.join(": ")
}

impl Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0.title)?;

        if let Some(detail) = self.detail() {
            write!(f, " ({detail})")?;
        }

        Ok(())
    }
}

impl std::error::Error for Error {}

pub trait Ctx<T> {
    fn ctx(self, op: &'static str) -> Result<T>;
}

impl<T> Ctx<T> for std::result::Result<T, sqlx::Error> {
    fn ctx(self, op: &'static str) -> Result<T> {
        self.map_err(|source| {
            Error::bare()
                .title("database request failed")
                .caused(Cause::Store {
                    op,
                    source: Box::new(source),
                })
        })
    }
}

impl<T> Ctx<T> for std::result::Result<T, serenity::Error> {
    fn ctx(self, op: &'static str) -> Result<T> {
        self.map_err(|source| {
            Error::bare()
                .title("discord request failed")
                .caused(Cause::Discord {
                    op,
                    source: Box::new(source),
                })
        })
    }
}

impl<T> Ctx<T> for Option<T> {
    fn ctx(self, context: &'static str) -> Result<T> {
        self.ok_or_else(|| Error::internal(context))
    }
}
