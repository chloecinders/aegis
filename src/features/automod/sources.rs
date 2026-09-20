use std::borrow::Cow;
use std::sync::LazyLock;

use regex::Regex;
use serenity::all::{Attachment, Message};

use crate::features::automod::rule::{Measure, Rule, Source};
use crate::platform::http::{Failure, Http};
use crate::platform::ocr;

static LINK_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"https?://\S+").expect("the link shape is a literal"));

static INVITE_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)(?:discord\.(?:gg|com/invite)|discordapp\.com/invite)/[a-z0-9-]{2,32}")
        .expect("the invite shape is a literal")
});

pub fn text(msg: &Message, source: Source) -> Option<Cow<'_, str>> {
    let extracted: Cow<'_, str> = match source {
        Source::Content => Cow::Borrowed(msg.content.as_str()),
        Source::Filename => Cow::Owned(
            msg.attachments
                .iter()
                .map(|attachment| attachment.filename.as_str())
                .collect::<Vec<&str>>()
                .join("\n"),
        ),
        Source::Mimetype => Cow::Owned(
            msg.attachments
                .iter()
                .filter_map(|attachment| attachment.content_type.as_deref())
                .collect::<Vec<&str>>()
                .join("\n"),
        ),
        Source::Embed => Cow::Owned(
            msg.embeds
                .iter()
                .flat_map(|embed| {
                    embed
                        .title
                        .iter()
                        .chain(embed.description.iter())
                        .chain(embed.url.iter())
                        .chain(embed.footer.iter().map(|footer| &footer.text))
                        .chain(embed.author.iter().map(|author| &author.name))
                        .cloned()
                        .chain(
                            embed
                                .fields
                                .iter()
                                .flat_map(|field| [field.name.clone(), field.value.clone()]),
                        )
                })
                .collect::<Vec<String>>()
                .join("\n"),
        ),
        Source::Username => Cow::Owned(
            [
                Some(msg.author.name.as_str()),
                msg.author.global_name.as_deref(),
                msg.member
                    .as_ref()
                    .and_then(|member| member.nick.as_deref()),
            ]
            .into_iter()
            .flatten()
            .collect::<Vec<&str>>()
            .join("\n"),
        ),
        Source::Image | Source::Join => Cow::Borrowed(""),
    };

    match extracted.trim().is_empty() {
        true => None,
        false => Some(extracted),
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Readable {
    pub url: String,
    pub full: Option<String>,
}

impl Readable {
    pub async fn fetch(&self, http: &Http) -> Result<Vec<u8>, Failure> {
        let asked = http.bytes(&self.url, 8 * 1024 * 1024).await;

        let Some(full) = self.full.as_deref() else {
            return asked;
        };

        match asked {
            Ok(bytes) => Ok(bytes),
            Err(_) => http.bytes(full, 8 * 1024 * 1024).await,
        }
    }
}

pub fn shrunk(attachment: &Attachment) -> Readable {
    let scaled = attachment
        .width
        .zip(attachment.height)
        .and_then(|(width, height)| ocr::scale(width, height));

    let Some((width, height)) = scaled else {
        return Readable {
            url: attachment.url.clone(),
            full: None,
        };
    };

    let joined = match attachment.proxy_url.contains('?') {
        true => '&',
        false => '?',
    };

    Readable {
        url: format!(
            "{}{joined}width={width}&height={height}",
            attachment.proxy_url
        ),
        full: Some(attachment.url.clone()),
    }
}

pub fn readable(msg: &Message) -> Vec<Readable> {
    let mut wanted: Vec<Readable> = Vec::new();

    let worth_fetching = msg.attachments.iter().filter(|attachment| {
        attachment.content_type.as_deref().is_some_and(|kind| {
            [
                "image/png",
                "image/jpeg",
                "image/webp",
                "image/gif",
                "image/bmp",
            ]
            .iter()
            .any(|readable| kind.starts_with(readable))
        }) && attachment.size as usize <= 8 * 1024 * 1024
            && attachment
                .width
                .zip(attachment.height)
                .is_none_or(|(width, height)| width >= 32 && height >= 32)
    });

    for attachment in worth_fetching {
        let target = shrunk(attachment);

        if !wanted.iter().any(|readable| readable.url == target.url) {
            wanted.push(target);
        }
    }

    wanted
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Counts {
    pub mentions: i64,
    pub links: i64,
    pub invites: i64,
    pub attachments: i64,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Required {
    pub links: bool,
    pub invites: bool,
}

pub fn needs(rules: &[Rule]) -> Required {
    let mut needed = Required::default();

    for condition in rules.iter().flat_map(|rule| rule.body.conditions.iter()) {
        match condition.measure {
            Measure::Links => needed.links = true,
            Measure::Invites => needed.invites = true,
            _ => {}
        }

        if needed.links && needed.invites {
            break;
        }
    }

    needed
}

pub fn counts(msg: &Message, needed: Required) -> Counts {
    Counts {
        mentions: (msg.mentions.len() + msg.mention_roles.len()) as i64,
        links: match needed.links {
            true => LINK_REGEX.find_iter(&msg.content).count() as i64,
            false => 0,
        },
        invites: match needed.invites {
            true => INVITE_REGEX.find_iter(&msg.content).count() as i64,
            false => 0,
        },
        attachments: msg.attachments.len() as i64,
    }
}
