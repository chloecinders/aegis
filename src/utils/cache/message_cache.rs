use std::{
    collections::{HashMap, VecDeque},
    mem::size_of,
};

use serenity::all::Message;
use tracing::error;

use crate::{
    ENCRYPTION_KEYS, SQL,
    utils::{
        cache::partials::{PartialAttachment, PartialMessage, PartialUser},
        encryption::{self, encrypt},
    },
};

pub struct MessageCache {
    sizes: HashMap<u64, usize>,
    messages: HashMap<u64, MessageQueue>,
    inserts: HashMap<u64, usize>,
}

impl MessageCache {
    pub fn new() -> Self {
        Self {
            sizes: HashMap::new(),
            messages: HashMap::new(),
            inserts: HashMap::new(),
        }
    }

    pub fn assign_count(&mut self, channel: u64, count: usize) {
        self.sizes.insert(channel, count);
        let entry = self
            .messages
            .entry(channel)
            .or_insert(MessageQueue::with_capacity(count));

        if entry.items.capacity() > count {
            while entry.items.len() > count {
                entry.pop();
            }

            entry.items.shrink_to(count);
        }
    }

    pub fn clear_inserts(&mut self) {
        self.inserts = self
            .inserts
            .clone()
            .into_keys()
            .map(|i| (i, 0_usize))
            .collect::<HashMap<_, _>>();
    }

    pub fn insert_message(&mut self, channel_id: u64, msg: Message) {
        let partial = PartialMessage::from(msg);
        self.insert(channel_id, partial);
    }

    pub fn insert(&mut self, channel_id: u64, message: PartialMessage) {
        *self.inserts.entry(channel_id).or_default() += 1;
        let queue_size = *self.sizes.entry(channel_id).or_insert(100);

        if queue_size > 0 {
            let queue = self.messages.entry(channel_id).or_default();

            if queue.len() >= queue_size {
                queue.pop();
            }

            queue.insert(message.clone());
        }

        tokio::spawn(async move {
            let guild_id = message.guild_id.unwrap_or(0);

            let content = message.content;

            let is_encrypted = {
                let lock = ENCRYPTION_KEYS.lock().await;
                lock.contains_key(&guild_id)
            };

            let content_bytes = if is_encrypted && !content.is_empty() {
                let lock = ENCRYPTION_KEYS.lock().await;
                if let Some(key) = lock.get(&guild_id) {
                    encrypt(key, &content).unwrap_or_else(|| content.into_bytes())
                } else {
                    content.into_bytes()
                }
            } else {
                content.into_bytes()
            };

            let attachments_json =
                serde_json::to_value(&message.attachment_urls).unwrap_or(serde_json::Value::Null);

            if let Err(err) = sqlx::query!(
                r#"
                INSERT INTO message_store
                (message_id, channel_id, guild_id, author_id, author_name, content, attachment_urls)
                VALUES ($1, $2, $3, $4, $5, $6, $7)
                ON CONFLICT (message_id) DO NOTHING
                "#,
                message.id as i64,
                channel_id as i64,
                guild_id as i64,
                message.author.id as i64,
                message.author.name,
                content_bytes,
                attachments_json
            )
            .execute(&*SQL)
            .await
            {
                error!("Failed to insert message into message_store: {err}");
            }
        });
    }

    pub fn insert_edit(guild_id: u64, message_id: u64, content: String) {
        tokio::spawn(async move {
            let is_encrypted = {
                let lock = ENCRYPTION_KEYS.lock().await;
                lock.contains_key(&guild_id)
            };

            let content_bytes = if is_encrypted && !content.is_empty() {
                let lock = ENCRYPTION_KEYS.lock().await;
                if let Some(key) = lock.get(&guild_id) {
                    encrypt(key, &content).unwrap_or_else(|| content.into_bytes())
                } else {
                    content.into_bytes()
                }
            } else {
                content.into_bytes()
            };

            if let Err(err) = sqlx::query!(
                r#"
                INSERT INTO message_edits (message_id, content)
                VALUES ($1, $2)
                "#,
                message_id as i64,
                content_bytes
            )
            .execute(&*SQL)
            .await
            {
                error!("Failed to insert message edit into message_edits: {err}");
            }
        });
    }

    pub fn get(&mut self, channel: u64, message: u64) -> Option<&PartialMessage> {
        let queue = self.messages.entry(channel).or_default();
        queue.get(message)
    }

    pub fn get_inserts(&self) -> HashMap<u64, usize> {
        self.inserts.clone()
    }

    pub fn get_sizes(&self) -> HashMap<u64, usize> {
        self.sizes.clone()
    }

    pub fn get_channel_len(&self, channel: u64) -> usize {
        self.messages
            .get(&channel)
            .map(|c| c.len())
            .unwrap_or_default()
    }

    pub fn byte_footprint(&self) -> usize {
        let mut size = size_of::<Self>();
        size += self.sizes.capacity() * (size_of::<u64>() + size_of::<usize>());
        size += self.inserts.capacity() * (size_of::<u64>() + size_of::<usize>());
        size += self.messages.capacity() * (size_of::<u64>() + size_of::<MessageQueue>());

        for queue in self.messages.values() {
            size += queue.items.capacity() * size_of::<PartialMessage>();

            for msg in &queue.items {
                size += msg.byte_footprint() - size_of::<PartialMessage>();
            }

            size += queue.index.capacity() * (size_of::<u64>() + size_of::<usize>());
        }
        size
    }

    pub async fn fetch(channel_id: u64, message_id: u64) -> Option<PartialMessage> {
        let record = sqlx::query!(
            r#"
            SELECT message_id, channel_id, guild_id, author_id, author_name, content, attachment_urls
            FROM message_store
            WHERE message_id = $1 AND channel_id = $2
            "#,
            message_id as i64,
            channel_id as i64
        )
        .fetch_optional(&*SQL)
        .await
        .ok()??;

        let guild_id = record.guild_id as u64;
        let content_bytes = record.content.unwrap_or_default();

        let is_encrypted = {
            let lock = ENCRYPTION_KEYS.lock().await;
            lock.contains_key(&guild_id)
        };

        let content = if is_encrypted && !content_bytes.is_empty() {
            let lock = ENCRYPTION_KEYS.lock().await;
            if let Some(key) = lock.get(&guild_id) {
                encryption::decrypt(key, &content_bytes)
                    .unwrap_or_else(|| String::from_utf8(content_bytes).unwrap_or_default())
            } else {
                String::from_utf8(content_bytes).unwrap_or_default()
            }
        } else {
            String::from_utf8(content_bytes).unwrap_or_default()
        };

        let attachment_urls: Vec<PartialAttachment> =
            serde_json::from_value(record.attachment_urls.unwrap_or(serde_json::Value::Null))
                .unwrap_or_default();

        Some(PartialMessage {
            id: record.message_id as u64,
            guild_id: Some(guild_id),
            channel_id: record.channel_id as u64,
            content,
            author: PartialUser {
                id: record.author_id as u64,
                name: record.author_name,
                bot: false,
            },
            attachment_urls,
        })
    }
}

struct MessageQueue {
    pub items: VecDeque<PartialMessage>,
    index: HashMap<u64, usize>,
}

impl MessageQueue {
    fn with_capacity(capacity: usize) -> Self {
        Self {
            items: VecDeque::with_capacity(capacity),
            index: Default::default(),
        }
    }

    fn insert(&mut self, msg: PartialMessage) {
        let id = msg.id;

        if let Some(&idx) = self.index.get(&id) {
            self.items[idx] = msg;
        } else {
            self.index.insert(id, self.items.len());
            self.items.push_back(msg);
        }
    }

    fn len(&self) -> usize {
        self.items.len()
    }

    fn get(&self, id: u64) -> Option<&PartialMessage> {
        self.index.get(&id).map(|&i| &self.items[i])
    }

    fn pop(&mut self) {
        if let Some(msg) = self.items.pop_front() {
            self.index.remove(&msg.id);

            for (i, m) in self.items.iter().enumerate() {
                self.index.insert(m.id, i);
            }
        }
    }
}

impl Default for MessageQueue {
    fn default() -> Self {
        Self {
            items: VecDeque::with_capacity(100),
            index: Default::default(),
        }
    }
}
