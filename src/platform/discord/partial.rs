use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serenity::all::Message;

use crate::domain::Snowflake;

#[derive(Clone, Debug)]
pub struct PartialUser {
    pub id: Snowflake,
    pub name: String,
    pub display_name: Option<String>,
    pub avatar_url: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PartialAttachment {
    #[serde(alias = "filename")]
    pub name: String,
    pub url: String,
    #[serde(default)]
    pub size: u32,
    #[serde(default)]
    pub content_type: Option<String>,
}

#[derive(Clone, Debug)]
pub struct PartialMessage {
    pub id: Snowflake,
    pub guild_id: Option<Snowflake>,
    pub channel_id: Snowflake,
    pub referenced_message_id: Option<Snowflake>,
    pub content: String,
    pub author: PartialUser,
    pub attachments: Vec<PartialAttachment>,
    pub created_at: DateTime<Utc>,
}

fn display_name(message: &Message) -> String {
    let preferred = message
        .member
        .as_ref()
        .and_then(|member| member.nick.as_ref())
        .or(message.author.global_name.as_ref());

    match preferred {
        Some(name) if name.as_str() != message.author.name.as_str() => {
            format!("{name} ({})", message.author.name)
        }
        _ => message.author.name.to_string(),
    }
}

impl From<&Message> for PartialMessage {
    fn from(message: &Message) -> Self {
        let display_name = display_name(message);
        let avatar_url = message
            .author
            .avatar_url()
            .or_else(|| Some(message.author.default_avatar_url()));

        Self {
            id: message.id.get(),
            guild_id: message.guild_id.map(|guild| guild.get()),
            channel_id: message.channel_id.get(),
            referenced_message_id: message
                .message_reference
                .as_ref()
                .and_then(|reference| reference.message_id)
                .map(|id| id.get()),
            content: message.content.to_string(),
            author: PartialUser {
                id: message.author.id.get(),
                name: message.author.name.to_string(),
                display_name: Some(display_name),
                avatar_url,
            },
            attachments: message
                .attachments
                .iter()
                .map(|attachment| PartialAttachment {
                    name: attachment.filename.to_string(),
                    url: attachment.url.to_string(),
                    size: attachment.size,
                    content_type: attachment.content_type.as_ref().map(ToString::to_string),
                })
                .collect(),
            created_at: *message.timestamp,
        }
    }
}
