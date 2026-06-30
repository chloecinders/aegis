use serde::{Deserialize, Serialize};
use serenity::all::{CacheHttp, Message, User};

#[derive(Clone, Debug)]
pub struct PartialUser {
    pub id: u64,
    pub name: String,
    pub display_name: Option<String>,
    pub avatar_url: Option<String>,
    pub bot: bool,
}

impl PartialUser {
    pub fn byte_footprint(&self) -> usize {
        std::mem::size_of::<Self>()
            + self.name.capacity()
            + self
                .display_name
                .as_ref()
                .map(|s| s.capacity())
                .unwrap_or(0)
            + self.avatar_url.as_ref().map(|s| s.capacity()).unwrap_or(0)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PartialAttachment {
    #[serde(alias = "filename")]
    pub name: String,
    pub url: String,
}

impl PartialAttachment {
    pub fn byte_footprint(&self) -> usize {
        std::mem::size_of::<Self>() + self.name.capacity() + self.url.capacity()
    }
}

#[derive(Clone, Debug)]
pub struct PartialMessage {
    pub id: u64,
    pub guild_id: Option<u64>,
    pub channel_id: u64,
    pub content: String,
    pub author: PartialUser,
    pub attachment_urls: Vec<PartialAttachment>,
    pub embeds: Vec<serde_json::Value>,
}

impl From<Message> for PartialMessage {
    fn from(value: Message) -> Self {
        let display_name = if let Some(member) = &value.member
            && let Some(nick) = &member.nick
        {
            if nick != &value.author.name {
                format!("{} ({})", nick, value.author.name)
            } else {
                value.author.name.clone()
            }
        } else if let Some(g) = &value.author.global_name {
            if g != &value.author.name {
                format!("{} ({})", g, value.author.name)
            } else {
                value.author.name.clone()
            }
        } else {
            value.author.name.clone()
        };

        let avatar_url = value
            .author
            .avatar_url()
            .or_else(|| Some(value.author.default_avatar_url()));

        Self {
            id: value.id.get(),
            guild_id: value.guild_id.map(|g| g.get()),
            channel_id: value.channel_id.get(),
            content: value.content,
            author: PartialUser {
                id: value.author.id.get(),
                name: value.author.name,
                display_name: Some(display_name),
                avatar_url,
                bot: value.author.bot,
            },
            attachment_urls: value
                .attachments
                .into_iter()
                .map(|a| PartialAttachment {
                    name: a.filename,
                    url: a.url,
                })
                .collect(),
            embeds: value
                .embeds
                .into_iter()
                .map(|e| serde_json::to_value(&e).unwrap_or(serde_json::Value::Null))
                .collect(),
        }
    }
}

impl PartialMessage {
    pub fn byte_footprint(&self) -> usize {
        let mut size = std::mem::size_of::<Self>();
        size += self.content.capacity();
        size += self.author.byte_footprint() - std::mem::size_of::<PartialUser>();
        size += self.attachment_urls.capacity() * std::mem::size_of::<PartialAttachment>();

        for attachment in &self.attachment_urls {
            size += attachment.byte_footprint() - std::mem::size_of::<PartialAttachment>();
        }

        size += self.embeds.capacity() * std::mem::size_of::<serde_json::Value>();

        size
    }

    // pub async fn to_message(&self, ctx: &impl CacheHttp) -> Option<Message> {
    //     let mut current = ctx.http().get_message(self.channel_id.into(), self.id.into()).await.ok()?;
    //     current.content = self.content.clone();
    //     Some(current)
    // }
}

impl PartialUser {
    pub async fn to_user(&self, ctx: &impl CacheHttp) -> Option<User> {
        ctx.http().get_user(self.id.into()).await.ok()
    }
}

impl PartialAttachment {
    pub async fn download(&self) -> Result<Vec<u8>, reqwest::Error> {
        let reqwest = reqwest::Client::new();
        let bytes = reqwest.get(&self.url).send().await?.bytes().await?;
        Ok(bytes.to_vec())
    }
}
