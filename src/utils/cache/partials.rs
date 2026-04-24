use serde::{Deserialize, Serialize};
use serenity::all::{CacheHttp, Message, User};

#[derive(Clone, Debug)]
pub struct PartialUser {
    pub id: u64,
    pub name: String,
    pub bot: bool,
}

impl PartialUser {
    pub fn byte_footprint(&self) -> usize {
        std::mem::size_of::<Self>() + self.name.capacity()
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PartialAttachment {
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
}

impl From<Message> for PartialMessage {
    fn from(value: Message) -> Self {
        Self {
            id: value.id.get(),
            guild_id: value.guild_id.map(|g| g.get()),
            channel_id: value.channel_id.get(),
            content: value.content,
            author: PartialUser {
                id: value.author.id.get(),
                name: value.author.name,
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
