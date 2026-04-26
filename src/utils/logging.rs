use chrono::DateTime;
use serde::{Deserialize, Serialize};
use serenity::all::{CacheHttp, ChannelId, CreateMessage, GuildId};
use tracing::warn;

use crate::{ENCRYPTION_KEYS, GUILD_SETTINGS, SQL, utils::encryption::encrypt};

#[derive(Clone, Debug)]
pub struct LogContext {
    pub target_id: u64,
    pub moderator_id: u64,
    pub db_id: Option<String>,
    pub content: Option<String>,
}

#[derive(Hash, Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum LogType {
    MemberModeration,
    MemberUpdate,
    ActionUpdate,
    MessageUpdate,
    AegisAnnouncements,
    AvatarUpdate,
    Channels,
    Roles,
}

impl LogType {
    pub fn title(&self) -> String {
        String::from(match self {
            LogType::MemberModeration => "Member Moderation",
            LogType::MemberUpdate => "Member Update",
            LogType::ActionUpdate => "Action Update",
            LogType::MessageUpdate => "Message Delete",
            LogType::AegisAnnouncements => "Aegis Announcements",
            LogType::AvatarUpdate => "Member Avatar Updates",
            LogType::Channels => "Channels",
            LogType::Roles => "Roles",
        })
    }

    pub fn description(&self) -> String {
        String::from(match self {
            LogType::MemberModeration => "New warns, bans, mutes, etc.",
            LogType::MemberUpdate => "Nickname, role changes",
            LogType::ActionUpdate => "Modeartion action duration/reason change",
            LogType::MessageUpdate => "Message deletions and edits",
            LogType::AegisAnnouncements => "Scheduled bot downtime, updates",
            LogType::AvatarUpdate => "Avatar updates (Can get very spammy in large servers!)",
            LogType::Channels => "Channel create/update/delete events",
            LogType::Roles => "Role create/update/delete events",
        })
    }

    pub fn all() -> Vec<LogType> {
        vec![
            LogType::MemberModeration,
            LogType::MemberUpdate,
            LogType::ActionUpdate,
            LogType::MessageUpdate,
            LogType::AegisAnnouncements,
            LogType::Channels,
            LogType::Roles,
        ]
    }

    pub async fn channel_id(&self, guild: GuildId) -> Option<ChannelId> {
        let mut lock = GUILD_SETTINGS.lock().await;
        let settings = lock.get(guild.get()).await.ok()?;

        settings
            .log
            .log_channel_ids
            .get(self)
            .map(|c| ChannelId::new(*c))
    }
}

pub async fn guild_log(
    http: impl CacheHttp,
    log_type: LogType,
    guild: GuildId,
    msg: CreateMessage,
    context: Option<LogContext>,
) {
    let Some(channel) = log_type.channel_id(guild).await else {
        return;
    };

    match channel.send_message(http, msg).await {
        Ok(message) => {
            let mut final_content_bytes = None;

            if let Some(mut ctx) = context {
                let is_encrypted = {
                    let lock = ENCRYPTION_KEYS.lock().await;
                    lock.contains_key(&guild.get())
                };

                if let Some(content) = ctx.content.take() {
                    let mut bytes = content.into_bytes();

                    if is_encrypted && !bytes.is_empty() {
                        let lock = ENCRYPTION_KEYS.lock().await;
                        if let Some(key) = lock.get(&guild.get()) {
                            if let Ok(content_str) = String::from_utf8(bytes.clone()) {
                                if let Some(encrypted) = encrypt(key, &content_str) {
                                    bytes = encrypted;
                                }
                            }
                        }
                    }
                    final_content_bytes = Some(bytes);
                }

                if let Err(err) = sqlx::query(
                    "INSERT INTO log_messages_context (message_id, guild_id, target_id, moderator_id, db_id, content) VALUES ($1, $2, $3, $4, $5, $6)",
                )
                .bind(message.id.get() as i64)
                .bind(guild.get() as i64)
                .bind(ctx.target_id as i64)
                .bind(ctx.moderator_id as i64)
                .bind(ctx.db_id)
                .bind(final_content_bytes)
                .execute(&*SQL).await {
                    warn!("Cannot save log message context into log_messages_context; err = {err}");
                }
            }
        }
        Err(err) => {
            warn!("Cannot not send log message; err = {err}");
        }
    }
}

pub fn snowflake_to_timestamp(snowflake: u64) -> chrono::DateTime<chrono::Utc> {
    let discord_epoch: i64 = 1420070400000;
    let timestamp = ((snowflake >> 22) as i64) + discord_epoch;

    DateTime::from_naive_utc_and_offset(
        DateTime::from_timestamp_millis(timestamp)
            .unwrap()
            .naive_utc(),
        chrono::Utc,
    )
}
