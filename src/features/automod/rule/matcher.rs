use regex::Regex;
use serde::de::Error as _;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::platform::text::fuzzy;

#[derive(Clone, Debug)]
pub struct Pattern(Regex);

impl Pattern {
    pub fn new(pattern: &str) -> Option<Self> {
        Regex::new(pattern).ok().map(Pattern)
    }

    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }

    pub fn is_match(&self, text: &str) -> bool {
        self.0.is_match(text)
    }
}

impl Serialize for Pattern {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for Pattern {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let pattern = String::deserialize(deserializer)?;

        Pattern::new(&pattern).ok_or_else(|| D::Error::custom("unparseable regex"))
    }
}

#[derive(Clone, Debug)]
pub struct Wildcard {
    text: String,
    pattern: Pattern,
}

impl Wildcard {
    pub fn new(text: &str) -> Option<Self> {
        let word = |ch: char| ch.is_alphanumeric() || ch == '_';
        let mut compiled = String::from("(?i)");

        if text.starts_with(word) {
            compiled.push_str(r"\b");
        }

        for ch in text.chars() {
            match ch {
                '*' => compiled.push_str(".*"),
                '?' => compiled.push('.'),
                _ => compiled.push_str(&regex::escape(ch.encode_utf8(&mut [0; 4]))),
            }
        }

        if text.ends_with(word) {
            compiled.push_str(r"\b");
        }

        Pattern::new(&compiled).map(|pattern| Wildcard {
            text: text.to_string(),
            pattern,
        })
    }

    pub fn as_str(&self) -> &str {
        &self.text
    }
}

impl Serialize for Wildcard {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for Wildcard {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let text = String::deserialize(deserializer)?;

        Wildcard::new(&text).ok_or_else(|| D::Error::custom("unparseable wildcard"))
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum Matcher {
    Literal { text: String },
    Regex { pattern: Pattern },
    Wildcard { pattern: Wildcard },
}

impl Matcher {
    pub fn parse(raw: &str) -> Result<Self, &'static str> {
        if raw.len() >= 2
            && let Some(body) = raw
                .strip_prefix('/')
                .and_then(|rest| rest.strip_suffix('/'))
        {
            return Pattern::new(body)
                .map(|pattern| Matcher::Regex { pattern })
                .ok_or("regex does not compile");
        }

        if raw.len() >= 2
            && let Some(body) = raw
                .strip_prefix('|')
                .and_then(|rest| rest.strip_suffix('|'))
        {
            return match body.is_empty() {
                true => Err("empty pattern"),
                false => Wildcard::new(body)
                    .map(|pattern| Matcher::Wildcard { pattern })
                    .ok_or("wildcard can not compile"),
            };
        }

        match raw.is_empty() {
            true => Err("empty pattern"),
            false => Ok(Matcher::Literal {
                text: raw.to_string(),
            }),
        }
    }

    pub fn test(&self, read: &fuzzy::Haystack) -> bool {
        match self {
            Matcher::Literal { text: needle } => fuzzy::contains_loose(needle, read, 0.95),
            Matcher::Regex { pattern } => pattern.is_match(read.text()),
            Matcher::Wildcard { pattern } => pattern.pattern.is_match(read.text()),
        }
    }

    pub fn render(&self) -> String {
        match self {
            Matcher::Literal { text } => format!("{text:?}"),
            Matcher::Regex { pattern } => format!("/{}/", pattern.as_str()),
            Matcher::Wildcard { pattern } => format!("|{}|", pattern.as_str()),
        }
    }
}
