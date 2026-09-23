use std::fmt::{self, Display};

use serde::{Deserialize, Serialize};

#[derive(
    Clone, Copy, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize,
)]
#[serde(rename_all = "lowercase")]
pub enum Source {
    Image,
    #[default]
    Content,
    Filename,
    Mimetype,
    Embed,
    Username,
    Join,
    Message,
}

impl Source {
    pub fn as_str(&self) -> &'static str {
        match self {
            Source::Image => "image",
            Source::Content => "content",
            Source::Filename => "filename",
            Source::Mimetype => "mimetype",
            Source::Embed => "embed",
            Source::Username => "username",
            Source::Join => "join",
            Source::Message => "message",
        }
    }

    pub fn parse(raw: &str) -> Option<Self> {
        match raw {
            "image" => Some(Source::Image),
            "content" => Some(Source::Content),
            "filename" => Some(Source::Filename),
            "mimetype" => Some(Source::Mimetype),
            "embed" => Some(Source::Embed),
            "username" => Some(Source::Username),
            "join" => Some(Source::Join),
            "message" => Some(Source::Message),
            _ => None,
        }
    }

    pub fn yields_text(&self) -> bool {
        !matches!(self, Source::Join | Source::Message)
    }

    pub fn is_expensive(&self) -> bool {
        matches!(self, Source::Image)
    }
}

impl Display for Source {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}
