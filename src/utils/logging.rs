use chrono::DateTime;
use serde::{Deserialize, Serialize};
use serenity::all::{
    CacheHttp, ChannelId, Context, CreateEmbed, CreateMessage, EditMessage, GuildId, MessageId,
};
use tracing::warn;

use crate::{
    ENCRYPTION_KEYS, GUILD_SETTINGS, SQL, constants::BRAND_BLUE, database::ActionType,
    utils::encryption::encrypt,
};

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
    MemberJoinLeave,
    ActionUpdate,
    MessageUpdate,
    AegisAnnouncements,
    AvatarUpdate,
    Channels,
    Roles,
    VoiceActivity,
    Expressions,
}

impl LogType {
    pub fn title(&self) -> String {
        String::from(match self {
            LogType::MemberModeration => "Member Moderation",
            LogType::MemberUpdate => "Member Update",
            LogType::MemberJoinLeave => "Member Join/Leave",
            LogType::ActionUpdate => "Action Update",
            LogType::MessageUpdate => "Message Delete",
            LogType::AegisAnnouncements => "Aegis Announcements",
            LogType::AvatarUpdate => "Member Avatar Updates",
            LogType::Channels => "Channels",
            LogType::Roles => "Roles",
            LogType::VoiceActivity => "Voice Activity",
            LogType::Expressions => "Expressions",
        })
    }

    pub fn description(&self) -> String {
        String::from(match self {
            LogType::MemberModeration => "New warns, bans, mutes, etc.",
            LogType::MemberUpdate => "Nickname, role changes",
            LogType::MemberJoinLeave => "Member joins and leaves",
            LogType::ActionUpdate => "Modeartion action duration/reason change",
            LogType::MessageUpdate => "Message deletions and edits",
            LogType::AegisAnnouncements => "Scheduled bot downtime, updates",
            LogType::AvatarUpdate => "Avatar updates (Can get very spammy in large servers!)",
            LogType::Channels => "Channel create/update/delete events",
            LogType::Roles => "Role create/update/delete events",
            LogType::VoiceActivity => "Voice joins, leaves, moves, mutes, deafens",
            LogType::Expressions => "Emoji/sticker create, update, delete events",
        })
    }

    pub fn all() -> Vec<LogType> {
        vec![
            LogType::MemberModeration,
            LogType::MemberUpdate,
            LogType::MemberJoinLeave,
            LogType::ActionUpdate,
            LogType::MessageUpdate,
            LogType::AegisAnnouncements,
            LogType::Channels,
            LogType::Roles,
            LogType::VoiceActivity,
            LogType::Expressions,
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

pub async fn update_guild_log(ctx: &Context, guild_id: GuildId, db_id: &str) {
    let record = match sqlx::query!(
        r#"
        SELECT id, type as "type!: ActionType", moderator_id, user_id, created_at, expires_at, reason, note
        FROM actions
        WHERE guild_id = $1 AND id = $2
        "#,
        guild_id.get() as i64,
        db_id
    )
    .fetch_optional(&*SQL)
    .await
    {
        Ok(Some(rec)) => rec,
        _ => return,
    };

    let message_id = match sqlx::query!(
        "SELECT message_id FROM log_messages_context WHERE guild_id = $1 AND db_id = $2",
        guild_id.get() as i64,
        db_id
    )
    .fetch_optional(&*SQL)
    .await
    {
        Ok(Some(rec)) => MessageId::new(rec.message_id as u64),
        _ => return,
    };

    let mut channel_ids = Vec::new();
    if let Some(ch) = LogType::MemberModeration.channel_id(guild_id).await {
        channel_ids.push(ch);
    }
    {
        let mut lock = GUILD_SETTINGS.lock().await;
        if let Ok(settings) = lock.get(guild_id.get()).await {
            for &ch_id in settings.log.log_channel_ids.values() {
                let ch = ChannelId::new(ch_id);
                if !channel_ids.contains(&ch) {
                    channel_ids.push(ch);
                }
            }
        }
    }

    let mut found_msg = None;
    for ch in channel_ids {
        if let Ok(m) = ctx.http.get_message(ch, message_id).await {
            found_msg = Some(m);
            break;
        }
    }

    let Some(mut msg) = found_msg else {
        return;
    };

    let title = match record.r#type {
        ActionType::Warn => "**MEMBER WARNED**",
        ActionType::Kick => "**MEMBER KICKED**",
        ActionType::Ban => "**MEMBER BANNED**",
        ActionType::Softban => "**MEMBER SOFTBANNED**",
        ActionType::Mute => "**MEMBER MUTED**",
        ActionType::Unban => "**MEMBER UNBANNED**",
        ActionType::Unmute => "**MEMBER UNMUTED**",
        ActionType::Log => "**MEMBER LOGGED**",
    };

    let mut header = format!(
        "-# Log ID: `{db_id}` | Actor: <@{}> | Target: <@{}>",
        record.moderator_id, record.user_id
    );

    if matches!(record.r#type, ActionType::Mute | ActionType::Ban) {
        let duration = record
            .expires_at
            .map(|e| e.signed_duration_since(record.created_at))
            .unwrap_or_else(chrono::Duration::zero);

        let time_string = if duration > chrono::Duration::zero() {
            let (time, mut unit) = match duration {
                _ if duration.num_days() != 0 => (duration.num_days(), String::from("day")),
                _ if duration.num_hours() != 0 => (duration.num_hours(), String::from("hour")),
                _ if duration.num_minutes() != 0 => {
                    (duration.num_minutes(), String::from("minute"))
                }
                _ if duration.num_seconds() != 0 => {
                    (duration.num_seconds(), String::from("second"))
                }
                _ => (0, String::new()),
            };

            if time > 1 {
                unit += "s";
            }
            format!("{time} {unit}")
        } else {
            String::from("permanent")
        };

        header.push_str(&format!(" | Duration: {time_string}"));
    }

    if let Some(old_desc) = msg.embeds.first().and_then(|e| e.description.as_ref()) {
        if let Some(pos) = old_desc.find(" | Cleared ") {
            if let Some(end_pos) = old_desc[pos..].find('\n') {
                header.push_str(&old_desc[pos..pos + end_pos]);
            } else {
                header.push_str(&old_desc[pos..]);
            }
        }
    }

    let note_suffix = record
        .note
        .as_deref()
        .map(|n| format!("\n-# {n}"))
        .unwrap_or_default();

    let new_desc = format!(
        "{title}\n{header}\n```\n{}\n```{note_suffix}",
        record.reason
    );

    let embed = CreateEmbed::new().description(new_desc).color(BRAND_BLUE);

    let _ = msg.edit(ctx, EditMessage::new().embed(embed)).await;
}
