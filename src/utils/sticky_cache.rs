use std::collections::{HashMap, HashSet};

use serenity::all::{Color, CreateEmbed, CreateMessage};
use sqlx::Row;
use tracing::warn;

use crate::SQL;

#[derive(Debug, Clone)]
pub struct StickyMessage {
    pub content: String,
    pub title: Option<String>,
    pub color: Option<Color>,
    pub last_message_id: Option<u64>,
}

impl StickyMessage {
    pub fn build_message(&self) -> CreateMessage {
        if self.title.is_none() && self.color.is_none() {
            CreateMessage::new().content(&self.content)
        } else {
            let color = self.color.unwrap_or(crate::constants::BRAND_BLUE);
            let description = if let Some(t) = &self.title {
                format!("**{t}**\n{}", self.content)
            } else {
                self.content.clone()
            };
            let embed = CreateEmbed::new().description(description).color(color);
            CreateMessage::new().add_embed(embed)
        }
    }
}

#[derive(Default)]
pub struct StickyCache {
    entries: HashMap<u64, StickyMessage>,
    pending_timers: HashSet<u64>,
}

impl StickyCache {
    pub fn new() -> Self {
        Self {
            entries: HashMap::new(),
            pending_timers: HashSet::new(),
        }
    }

    pub async fn populate_from_db(&mut self) {
        let res = sqlx::query(
            "SELECT channel_id, content, title, color, last_message_id FROM sticky_messages",
        )
        .fetch_all(&*SQL)
        .await;

        match res {
            Ok(rows) => {
                for r in rows {
                    let channel_id: i64 = r.get("channel_id");
                    let content: String = r.get("content");
                    let title: Option<String> = r.get("title");
                    let color: Option<i64> = r.get("color");
                    let last_message_id: Option<i64> = r.get("last_message_id");

                    self.entries.insert(
                        channel_id as u64,
                        StickyMessage {
                            content,
                            title,
                            color: color.map(|c| Color::new(c as u32)),
                            last_message_id: last_message_id.map(|id| id as u64),
                        },
                    );
                }
            }
            Err(e) => {
                warn!("Failed to populate sticky cache from db: {:?}", e);
            }
        }
    }

    pub fn set_sticky(
        &mut self,
        channel_id: u64,
        content: String,
        title: Option<String>,
        color: Option<Color>,
        last_message_id: Option<u64>,
    ) {
        self.entries.insert(
            channel_id,
            StickyMessage {
                content,
                title,
                color,
                last_message_id,
            },
        );
    }

    pub fn remove_sticky(&mut self, channel_id: u64) -> Option<StickyMessage> {
        self.entries.remove(&channel_id)
    }

    pub fn get(&self, channel_id: u64) -> Option<StickyMessage> {
        self.entries.get(&channel_id).cloned()
    }

    pub fn contains_channel(&self, channel_id: u64) -> bool {
        self.entries.contains_key(&channel_id)
    }

    pub fn is_timer_pending(&self, channel_id: u64) -> bool {
        self.pending_timers.contains(&channel_id)
    }

    pub fn set_timer_pending(&mut self, channel_id: u64) {
        self.pending_timers.insert(channel_id);
    }

    pub fn clear_timer_pending(&mut self, channel_id: u64) {
        self.pending_timers.remove(&channel_id);
    }

    pub fn update_last_message_id(&mut self, channel_id: u64, last_message_id: Option<u64>) {
        if let Some(entry) = self.entries.get_mut(&channel_id) {
            entry.last_message_id = last_message_id;
        }
    }
}
