use serenity::all::{Color, CreateEmbed};

use crate::platform::ui::tone::Tone;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Body {
    Prose(String),
    Quote(String),
}

impl Body {
    fn write(&self, out: &mut String) {
        out.push('\n');

        match self {
            Body::Prose(text) => out.push_str(text),
            Body::Quote(text) => out.push_str(&codeblock(text)),
        }
    }
}

pub fn codeblock(text: &str) -> String {
    format!("```\n{}\n```", text.replace("```", "\\`\\`\\`"))
}

pub fn code(value: &str) -> String {
    format!("`{}`", value.replace('`', "'"))
}

#[derive(Clone, Debug, Default)]
pub struct Embed {
    pub title: String,
    pub subtitles: Vec<String>,
    pub lead: Option<String>,
    pub body: Option<Body>,
    pub footnote: Option<String>,
    pub image: Option<String>,
    pub tone: Tone,
    pub color: Option<Color>,
}

impl Embed {
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            ..Default::default()
        }
    }

    pub fn subtitle(mut self, part: impl Into<String>) -> Self {
        self.subtitles.push(part.into());
        self
    }

    pub fn maybe_subtitle(mut self, part: Option<String>) -> Self {
        if let Some(part) = part {
            self.subtitles.push(part);
        }

        self
    }

    pub fn lead(mut self, lead: impl Into<String>) -> Self {
        self.lead = Some(lead.into());
        self
    }

    pub fn maybe_lead(mut self, lead: Option<String>) -> Self {
        self.lead = lead;
        self
    }

    pub fn body(mut self, body: impl Into<String>) -> Self {
        self.body = Some(Body::Prose(body.into()));
        self
    }

    pub fn quote(mut self, body: impl Into<String>) -> Self {
        self.body = Some(Body::Quote(body.into()));
        self
    }

    pub fn maybe_quote(mut self, body: Option<String>) -> Self {
        if let Some(body) = body {
            self.body = Some(Body::Quote(body));
        }

        self
    }

    pub fn footnote(mut self, footnote: impl Into<String>) -> Self {
        self.footnote = Some(footnote.into());
        self
    }

    pub fn maybe_footnote(mut self, footnote: Option<String>) -> Self {
        self.footnote = footnote;
        self
    }

    pub fn maybe_image(mut self, url: Option<String>) -> Self {
        self.image = url;
        self
    }

    pub fn tone(mut self, tone: Tone) -> Self {
        self.tone = tone;
        self
    }

    pub fn maybe_color(mut self, color: Option<Color>) -> Self {
        self.color = color;
        self
    }

    pub fn render(&self) -> String {
        let mut out = String::with_capacity(self.title.len() + 64);

        out.push_str("**");
        out.push_str(&self.title);
        out.push_str("**");

        if !self.subtitles.is_empty() {
            out.push('\n');
            out.push_str("-# ");
            out.push_str(&self.subtitles.join(" | "));
        }

        if let Some(lead) = &self.lead {
            out.push('\n');
            out.push_str(lead);
        }

        if let Some(body) = &self.body {
            body.write(&mut out);
        }

        if let Some(footnote) = &self.footnote {
            out.push_str("\n-# ");
            out.push_str(footnote);
        }

        out
    }

    pub fn build(&self) -> CreateEmbed<'_> {
        let built = CreateEmbed::new()
            .description(self.render())
            .color(self.color.unwrap_or_else(|| self.tone.color()));

        match &self.image {
            Some(url) => built.image(url, None),
            None => built,
        }
    }
}

pub fn mention(user: u64, name: Option<&str>) -> String {
    match name {
        Some(name) => format!("<@{user}> ({name})"),
        None => format!("<@{user}>"),
    }
}

pub fn channel_mention(channel: u64) -> String {
    format!("<#{channel}>")
}

pub fn role_mention(role: u64) -> String {
    format!("<@&{role}>")
}

pub fn jump(guild: u64, channel: u64, message: u64) -> String {
    format!("https://discord.com/channels/{guild}/{channel}/{message}")
}
