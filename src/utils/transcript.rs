use std::collections::HashMap;

use chrono::{TimeZone, Utc};
use serde::{Deserialize, Serialize};
use serenity::all::{ChannelId, Context, GuildId, Message, RoleId};
use sqlx::Row;

use crate::utils::{
    cache::{message_cache::MessageCache, partials::PartialAttachment},
    s3,
};

#[derive(Serialize, Deserialize)]
pub struct TranscriptData {
    pub meta: TranscriptMeta,
    pub messages: Vec<TranscriptMessage>,
}

#[derive(Serialize, Deserialize)]
pub struct TranscriptMeta {
    pub guild_name: String,
    pub channel_name: String,
    pub moderator: String,
    pub count: usize,
    pub timestamp: String,
}

#[derive(Serialize, Deserialize)]
pub struct TranscriptMessage {
    pub id: u64,
    pub timestamp_gutter: String,
    pub timestamp_header: String,
    pub author: TranscriptAuthor,
    pub content: String,
    pub attachments: Vec<TranscriptAttachment>,
    pub embeds: Vec<TranscriptEmbed>,
}

#[derive(Serialize, Deserialize)]
pub struct TranscriptAuthor {
    pub id: u64,
    pub name: String,
    pub avatar_url: Option<String>,
    pub is_bot: bool,
}

#[derive(Serialize, Deserialize)]
pub struct TranscriptAttachment {
    pub url: String,
    pub filename: String,
    pub is_image: bool,
}

#[derive(Serialize, Deserialize)]
pub struct TranscriptEmbed {
    pub border_color: String,
    pub author: Option<TranscriptEmbedAuthor>,
    pub title: Option<String>,
    pub url: Option<String>,
    pub description: Option<String>,
    pub fields: Vec<TranscriptEmbedField>,
    pub footer: Option<TranscriptEmbedFooter>,
}

#[derive(Serialize, Deserialize)]
pub struct TranscriptEmbedAuthor {
    pub name: String,
    pub icon_url: Option<String>,
}

#[derive(Serialize, Deserialize)]
pub struct TranscriptEmbedField {
    pub name: String,
    pub value: String,
    pub inline: bool,
}

#[derive(Serialize, Deserialize)]
pub struct TranscriptEmbedFooter {
    pub text: String,
    pub icon_url: Option<String>,
}

fn upload_to_s3_async(guild_id: u64, url: &str, ext: &str) -> String {
    let clean_ext = ext.split('?').next().unwrap_or(ext);
    let token = s3::random_token();
    let (key, predicted_url) = s3::get_predicted_url(guild_id, &token, clean_ext);
    let url_clone = url.to_string();
    tokio::spawn(async move {
        if let Ok(req) = reqwest::get(&url_clone).await {
            if let Ok(bytes) = req.bytes().await {
                let data = bytes.to_vec();
                let ct = s3::detect_content_type(&data);
                let _ = s3::upload_image_with_key(key, data, ct).await;
            }
        }
    });
    predicted_url
}

fn resolve_channel_name(ctx: &Context, guild_id: u64, id: u64) -> Option<String> {
    if let Some(ch) = ctx.cache.channel(ChannelId::new(id)) {
        return Some(format!("#{}", ch.name));
    }
    if let Some(guild) = ctx.cache.guild(GuildId::new(guild_id)) {
        if let Some(ch) = guild.channels.get(&ChannelId::new(id)) {
            return Some(format!("#{}", ch.name));
        }
    }
    None
}

fn resolve_role_name(ctx: &Context, guild_id: u64, id: u64) -> Option<String> {
    if let Some(role) = ctx.cache.role(GuildId::new(guild_id), RoleId::new(id)) {
        return Some(format!("@{}", role.name));
    }
    if let Some(guild) = ctx.cache.guild(GuildId::new(guild_id)) {
        if let Some(role) = guild.roles.get(&RoleId::new(id)) {
            return Some(format!("@{}", role.name));
        }
    }
    None
}

fn resolve_string_mentions(
    ctx: &Context,
    guild_id: u64,
    text: &str,
    m: Option<&Message>,
) -> String {
    let mut content = text.to_string();

    if let Some(msg) = m {
        for u in &msg.mentions {
            let mut name = u.name.clone();
            if let Some(g) = &u.global_name {
                name = g.clone();
            }
            if let Some(member) = ctx.cache.member(guild_id, u.id) {
                if let Some(nick) = &member.nick {
                    name = nick.clone();
                }
            }
            content = content.replace(&format!("<@{}>", u.id.get()), &format!("@{}", name));
            content = content.replace(&format!("<@!{}>", u.id.get()), &format!("@{}", name));
        }
        for c in &msg.mention_channels {
            content = content.replace(&format!("<#{}>", c.id.get()), &format!("#{}", c.name));
        }
        for r in &msg.mention_roles {
            if let Some(name) = resolve_role_name(ctx, guild_id, r.get()) {
                content = content.replace(&format!("<@&{}>", r.get()), &name);
            }
        }
    }

    if let Ok(re) = regex::Regex::new(r"<@!?(\d+)>") {
        content = re
            .replace_all(&content, |caps: &regex::Captures| {
                if let Ok(id) = caps[1].parse::<u64>() {
                    if let Some(member) = ctx.cache.member(guild_id, id) {
                        if let Some(nick) = &member.nick {
                            return format!("@{}", nick);
                        }
                    }
                    if let Some(user) = ctx.cache.user(id) {
                        if let Some(g) = &user.global_name {
                            return format!("@{}", g);
                        }
                        return format!("@{}", user.name);
                    }
                }
                caps[0].to_string()
            })
            .to_string();
    }

    if let Ok(re) = regex::Regex::new(r"<#(\d+)>") {
        content = re
            .replace_all(&content, |caps: &regex::Captures| {
                if let Ok(id) = caps[1].parse::<u64>() {
                    if let Some(name) = resolve_channel_name(ctx, guild_id, id) {
                        return name;
                    }
                }
                caps[0].to_string()
            })
            .to_string();
    }

    if let Ok(re) = regex::Regex::new(r"<@&(\d+)>") {
        content = re
            .replace_all(&content, |caps: &regex::Captures| {
                if let Ok(id) = caps[1].parse::<u64>() {
                    if let Some(name) = resolve_role_name(ctx, guild_id, id) {
                        return name;
                    }
                }
                caps[0].to_string()
            })
            .to_string();
    }

    content
}

fn resolve_json_mentions(
    ctx: &Context,
    guild_id: u64,
    val: &mut serde_json::Value,
    m: Option<&Message>,
) {
    match val {
        serde_json::Value::String(s) => {
            *s = resolve_string_mentions(ctx, guild_id, s, m);
        }
        serde_json::Value::Array(arr) => {
            for elem in arr {
                resolve_json_mentions(ctx, guild_id, elem, m);
            }
        }
        serde_json::Value::Object(obj) => {
            for (_, v) in obj.iter_mut() {
                resolve_json_mentions(ctx, guild_id, v, m);
            }
        }
        _ => {}
    }
}

pub async fn save_transcript(
    ctx: &Context,
    guild_id: u64,
    channel_id: u64,
    moderator: &str,
    messages: &[Message],
) -> String {
    let transcript_id = uuid::Uuid::new_v4().to_string();
    let mut message_ids = Vec::new();
    let mut avatar_cache: HashMap<u64, String> = HashMap::new();

    let channel_name = if let Some(ch) = ctx.cache.channel(channel_id) {
        format!("#{}", ch.name)
    } else if let Ok(ch) = ctx.http.get_channel(channel_id.into()).await
        && let Some(gch) = ch.guild()
    {
        format!("#{}", gch.name)
    } else {
        format!("#{channel_id}")
    };

    for m in messages {
        message_ids.push(m.id.get());
        let guild_id_val = m.guild_id.map(|g| g.get()).unwrap_or(guild_id);

        let display_name = if let Some(member) = &m.member
            && let Some(nick) = &member.nick
        {
            if nick != &m.author.name {
                format!("{} ({})", nick, m.author.name)
            } else {
                m.author.name.clone()
            }
        } else if let Some(g) = &m.author.global_name {
            if g != &m.author.name {
                format!("{} ({})", g, m.author.name)
            } else {
                m.author.name.clone()
            }
        } else {
            m.author.name.clone()
        };

        let avatar_url = avatar_cache.entry(m.author.id.get()).or_insert_with(|| {
            let raw_avatar = m
                .author
                .avatar_url()
                .unwrap_or_else(|| m.author.default_avatar_url());
            upload_to_s3_async(guild_id_val, &raw_avatar, "png")
        });

        let resolved_content = resolve_string_mentions(ctx, guild_id_val, &m.content, Some(m));

        let content_bytes = {
            let lock = crate::ENCRYPTION_KEYS.lock().await;
            if let Some(key) = lock.get(&guild_id_val) {
                crate::utils::encryption::encrypt(key, &resolved_content)
                    .unwrap_or_else(|| resolved_content.clone().into_bytes())
            } else {
                resolved_content.clone().into_bytes()
            }
        };

        let mut embeds_val =
            serde_json::to_value(&m.embeds).unwrap_or(serde_json::Value::Array(vec![]));
        resolve_json_mentions(ctx, guild_id_val, &mut embeds_val, Some(m));
        let embeds_str = serde_json::to_string(&embeds_val).unwrap_or_default();
        let embeds_bytes = {
            let lock = crate::ENCRYPTION_KEYS.lock().await;
            if let Some(key) = lock.get(&guild_id_val) {
                if !embeds_str.is_empty() && embeds_str != "[]" {
                    crate::utils::encryption::encrypt(key, &embeds_str)
                        .unwrap_or_else(|| embeds_str.into_bytes())
                } else {
                    embeds_str.into_bytes()
                }
            } else {
                embeds_str.into_bytes()
            }
        };

        let mut partial_attachments = Vec::new();
        for att in &m.attachments {
            let ext = att.filename.rsplit('.').next().unwrap_or("png");
            let s3_url = upload_to_s3_async(guild_id_val, &att.url, ext);
            partial_attachments.push(PartialAttachment {
                name: att.filename.clone(),
                url: s3_url,
            });
        }
        let attachments_json =
            serde_json::to_value(&partial_attachments).unwrap_or(serde_json::Value::Null);

        let _ = sqlx::query(
            r#"
            INSERT INTO message_store
            (message_id, channel_id, guild_id, author_id, author_name, author_display_name, author_avatar_url, content, attachment_urls, embeds)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
            ON CONFLICT (message_id) DO UPDATE SET
                author_display_name = EXCLUDED.author_display_name,
                author_avatar_url = EXCLUDED.author_avatar_url,
                attachment_urls = EXCLUDED.attachment_urls,
                content = EXCLUDED.content,
                embeds = EXCLUDED.embeds
            "#,
        )
        .bind(m.id.get() as i64)
        .bind(channel_id as i64)
        .bind(guild_id_val as i64)
        .bind(m.author.id.get() as i64)
        .bind(&m.author.name)
        .bind(&display_name)
        .bind(avatar_url.as_str())
        .bind(&content_bytes)
        .bind(attachments_json)
        .bind(&embeds_bytes)
        .execute(&*crate::SQL)
        .await;
    }

    let msg_ids_json =
        serde_json::to_value(&message_ids).unwrap_or(serde_json::Value::Array(vec![]));

    if let Err(err) = sqlx::query(
        r#"
        INSERT INTO transcripts (transcript_id, guild_id, channel_id, channel_name, moderator_name, message_ids)
        VALUES ($1, $2, $3, $4, $5, $6)
        "#,
    )
    .bind(&transcript_id)
    .bind(guild_id as i64)
    .bind(channel_id as i64)
    .bind(&channel_name)
    .bind(moderator)
    .bind(msg_ids_json)
    .execute(&*crate::SQL)
    .await
    {
        tracing::error!("Failed to save transcript {transcript_id} into DB: {err}");
    }

    transcript_id
}

pub async fn fetch_transcript_data(guild_id: u64, transcript_id: &str) -> Option<TranscriptData> {
    let rec = sqlx::query(
        r#"
        SELECT transcript_id, guild_id, channel_id, channel_name, moderator_name, message_ids, created_at
        FROM transcripts
        WHERE transcript_id = $1 AND guild_id = $2
        "#,
    )
    .bind(transcript_id)
    .bind(guild_id as i64)
    .fetch_optional(&*crate::SQL)
    .await
    .ok()??;

    let channel_id: u64 = rec.try_get::<i64, _>("channel_id").unwrap_or_default() as u64;
    let channel_name: String = rec
        .try_get("channel_name")
        .ok()
        .flatten()
        .unwrap_or_else(|| format!("#{channel_id}"));
    let moderator: String = rec.try_get("moderator_name").unwrap_or_default();
    let created_at_dt: chrono::DateTime<Utc> =
        rec.try_get("created_at").unwrap_or_else(|_| Utc::now());
    let message_ids_val: serde_json::Value = rec
        .try_get("message_ids")
        .unwrap_or(serde_json::Value::Null);
    let message_ids: Vec<u64> = serde_json::from_value(message_ids_val).unwrap_or_default();

    let mut transcript_messages = Vec::new();

    for msg_id in &message_ids {
        if let Some(partial) = MessageCache::fetch(channel_id, *msg_id).await {
            let timestamp_ms = (*msg_id >> 22) + 1420070400000;
            let dt = Utc
                .timestamp_millis_opt(timestamp_ms as i64)
                .single()
                .unwrap_or_else(Utc::now);
            let time_gutter = dt.format("%l:%M %p").to_string().trim().to_string();
            let time_header = dt.format("%b %d, %Y at %l:%M %p").to_string();

            let attachments = partial
                .attachment_urls
                .iter()
                .map(|att| {
                    let clean_url = att.url.split('?').next().unwrap_or(&att.url).to_lowercase();
                    let is_image = clean_url.ends_with(".png")
                        || clean_url.ends_with(".jpg")
                        || clean_url.ends_with(".jpeg")
                        || clean_url.ends_with(".gif")
                        || clean_url.ends_with(".webp");
                    TranscriptAttachment {
                        url: att.url.clone(),
                        filename: att.name.clone(),
                        is_image,
                    }
                })
                .collect();

            let embeds = partial
                .embeds
                .iter()
                .filter_map(|val| serde_json::from_value::<serenity::all::Embed>(val.clone()).ok())
                .map(|e| {
                    let border_color = match e.colour {
                        Some(c) => format!("#{:06x}", c.0),
                        None => "#202225".to_string(),
                    };

                    let author = e.author.as_ref().map(|auth| TranscriptEmbedAuthor {
                        name: auth.name.clone(),
                        icon_url: auth.icon_url.clone(),
                    });

                    let fields = e
                        .fields
                        .iter()
                        .map(|f| TranscriptEmbedField {
                            name: f.name.clone(),
                            value: f.value.clone(),
                            inline: f.inline,
                        })
                        .collect();

                    let footer = e.footer.as_ref().map(|ft| TranscriptEmbedFooter {
                        text: ft.text.clone(),
                        icon_url: ft.icon_url.clone(),
                    });

                    TranscriptEmbed {
                        border_color,
                        author,
                        title: e.title.clone(),
                        url: e.url.clone(),
                        description: e.description.clone(),
                        fields,
                        footer,
                    }
                })
                .collect();

            transcript_messages.push(TranscriptMessage {
                id: partial.id,
                timestamp_gutter: time_gutter,
                timestamp_header: time_header,
                author: TranscriptAuthor {
                    id: partial.author.id,
                    name: partial
                        .author
                        .display_name
                        .clone()
                        .unwrap_or_else(|| partial.author.name.clone()),
                    avatar_url: partial.author.avatar_url.clone(),
                    is_bot: partial.author.bot,
                },
                content: partial.content.clone(),
                attachments,
                embeds,
            });
        }
    }

    transcript_messages.sort_by_key(|m| m.id);

    Some(TranscriptData {
        meta: TranscriptMeta {
            guild_name: format!("Guild {guild_id}"),
            channel_name,
            moderator,
            count: transcript_messages.len(),
            timestamp: created_at_dt.format("%Y-%m-%d %H:%M:%S UTC").to_string(),
        },
        messages: transcript_messages,
    })
}
