use regex::Regex;
use serenity::all::{ChannelId, Context, Mentionable, Message, MessageId, UserId};
use tracing::warn;

use crate::{
    ENCRYPTION_KEYS, SQL,
    utils::{
        encryption::{decrypt, encrypt},
        s3,
    },
};

#[derive(Debug, Clone, Default)]
pub struct RefData {
    pub message_id: Option<u64>,
    pub channel_id: Option<u64>,
    pub guild_id: Option<u64>,
    pub author_id: Option<u64>,
    pub content: Option<String>,
    pub image_url: Option<String>,
}

impl RefData {
    pub fn is_empty(&self) -> bool {
        self.content.is_none() && self.image_url.is_none()
    }

    pub fn jump_url(&self) -> Option<String> {
        let channel_id = self.channel_id?;
        let message_id = self.message_id?;
        let guild_part = self
            .guild_id
            .map(|g| g.to_string())
            .unwrap_or_else(|| String::from("@me"));
        Some(format!(
            "https://discord.com/channels/{guild_part}/{channel_id}/{message_id}"
        ))
    }

    pub fn header(&self) -> Option<String> {
        let message_id = self.message_id?;
        let author_id = self.author_id?;
        let jump = self.jump_url()?;
        Some(format!(
            "-# ID: `{message_id}` [Jump]({jump}) | Author: {}",
            UserId::new(author_id).mention()
        ))
    }
}

fn extract_message_content(msg: &Message) -> Option<String> {
    if !msg.content.is_empty() {
        return Some(msg.content.clone());
    }
    if let Some(embed) = msg.embeds.first() {
        let desc = embed.description.clone().unwrap_or_default();
        if embed.kind.clone().unwrap_or_default() == "auto_moderation_message" {
            return Some(desc);
        }
        if desc.starts_with("**MESSAGE DELETED**") || desc.starts_with("**MESSAGE EDITED**") {
            return Some(String::from("<Stored context lost>"));
        }
    }
    None
}

pub async fn resolve_ref(
    ctx: &Context,
    msg: &Message,
    _action_id: &str,
    explicit_ref_url: Option<&str>,
) -> RefData {
    let guild_id = msg.guild_id.map(|g| g.get()).unwrap_or(0);

    let url = match explicit_ref_url {
        Some(u) => u,
        None => {
            let target_msg = msg.referenced_message.as_deref().unwrap_or(msg);

            if std::ptr::eq(target_msg, msg) {
                return RefData::default();
            }

            let content = extract_message_content(target_msg);
            let image_url = upload_attachments(guild_id, &target_msg.attachments).await;

            return RefData {
                message_id: Some(target_msg.id.get()),
                channel_id: Some(target_msg.channel_id.get()),
                guild_id: Some(guild_id),
                author_id: Some(target_msg.author.id.get()),
                content,
                image_url,
            };
        }
    };

    let re =
        Regex::new(r"https?://(?:canary\.|ptb\.)?discord(?:app)?\.com/channels/(\d+)/(\d+)/(\d+)")
            .unwrap();
    if let Some(caps) = re.captures(url) {
        if let (Ok(channel_id), Ok(message_id)) = (caps[2].parse::<u64>(), caps[3].parse::<u64>()) {
            if let Ok(fetched) = ChannelId::new(channel_id)
                .message(ctx, MessageId::new(message_id))
                .await
            {
                let content = extract_message_content(&fetched);
                let image_url = upload_attachments(guild_id, &fetched.attachments).await;
                return RefData {
                    message_id: Some(fetched.id.get()),
                    channel_id: Some(fetched.channel_id.get()),
                    guild_id: Some(guild_id),
                    author_id: Some(fetched.author.id.get()),
                    content,
                    image_url,
                };
            } else {
                warn!("reference: could not fetch Discord message {channel_id}/{message_id}");
            }
        }
    }

    let without_query = url.split('?').next().unwrap_or(url).to_lowercase();

    if matches!(
        without_query.rsplit('.').next().unwrap_or(""),
        "jpg" | "jpeg" | "png" | "gif" | "webp"
    ) {
        let token = s3::random_token();
        let ext = without_query
            .rsplit('.')
            .next()
            .unwrap_or("bin")
            .to_string();
        let (key, predicted_url) = s3::get_predicted_url(guild_id, &token, &ext);

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

        return RefData {
            image_url: Some(predicted_url),
            ..Default::default()
        };
    }

    RefData {
        content: Some(url.to_string()),
        ..Default::default()
    }
}

pub async fn save_ref(action_id: &str, ref_data: &RefData, guild_id: u64, reason_is_default: bool) {
    if ref_data.is_empty() {
        return;
    }

    if reason_is_default {
        if let Some(content) = &ref_data.content {
            if let Err(err) = sqlx::query!(
                "UPDATE actions SET reason = $1, updated_at = NOW() WHERE id = $2",
                content,
                action_id
            )
            .execute(&*SQL)
            .await
            {
                warn!("save_ref: failed to update reason; err = {err:?}");
            }
        }
    }

    let content_bytes: Option<Vec<u8>> = if let Some(plaintext) = &ref_data.content {
        let lock = ENCRYPTION_KEYS.lock().await;
        if let Some(key) = lock.get(&guild_id) {
            encrypt(key, plaintext).or_else(|| Some(plaintext.as_bytes().to_vec()))
        } else {
            Some(plaintext.as_bytes().to_vec())
        }
    } else {
        None
    };

    let message_id = ref_data.message_id.map(|v| v as i64);
    let channel_id = ref_data.channel_id.map(|v| v as i64);
    let author_id = ref_data.author_id.map(|v| v as i64);

    if let Err(err) = sqlx::query!(
        "INSERT INTO action_refs
             (action_id, ref_message_id, ref_channel_id, ref_author_id, ref_content, image_url)
         VALUES ($1, $2, $3, $4, $5, $6)
         ON CONFLICT (action_id) DO UPDATE
             SET ref_message_id = COALESCE(EXCLUDED.ref_message_id, action_refs.ref_message_id),
                 ref_channel_id = COALESCE(EXCLUDED.ref_channel_id, action_refs.ref_channel_id),
                 ref_author_id  = COALESCE(EXCLUDED.ref_author_id,  action_refs.ref_author_id),
                 ref_content    = COALESCE(EXCLUDED.ref_content,    action_refs.ref_content),
                 image_url      = COALESCE(EXCLUDED.image_url,      action_refs.image_url)",
        action_id,
        message_id,
        channel_id,
        author_id,
        content_bytes.as_deref(),
        ref_data.image_url.as_deref(),
    )
    .execute(&*SQL)
    .await
    {
        warn!("save_ref: failed to save action_refs; err = {err:?}");
    }
}

pub async fn get_ref(action_id: &str, guild_id: u64) -> Option<RefData> {
    let row = sqlx::query!(
        "SELECT ref_message_id, ref_channel_id, ref_author_id, ref_content, image_url
         FROM action_refs WHERE action_id = $1",
        action_id
    )
    .fetch_optional(&*SQL)
    .await
    .ok()??;

    let content = if let Some(bytes) = row.ref_content {
        let lock = ENCRYPTION_KEYS.lock().await;
        if let Some(key) = lock.get(&guild_id) {
            decrypt(key, &bytes).or_else(|| String::from_utf8(bytes).ok())
        } else {
            String::from_utf8(bytes).ok()
        }
    } else {
        None
    };

    Some(RefData {
        message_id: row.ref_message_id.map(|v| v as u64),
        channel_id: row.ref_channel_id.map(|v| v as u64),
        guild_id: Some(guild_id),
        author_id: row.ref_author_id.map(|v| v as u64),
        content,
        image_url: row.image_url,
    })
}

pub async fn update_ref(ctx: &Context, action_id: &str, new_ref_url: &str) -> bool {
    let row = sqlx::query!("SELECT guild_id FROM actions WHERE id = $1", action_id)
        .fetch_optional(&*SQL)
        .await
        .ok()
        .flatten();

    let guild_id = row.map(|r| r.guild_id as u64).unwrap_or(0);

    let new_data = resolve_ref(
        ctx,
        &serenity::all::Message::default(),
        action_id,
        Some(new_ref_url),
    )
    .await;

    if new_data.is_empty() {
        return false;
    }

    let content_bytes: Option<Vec<u8>> = if let Some(plaintext) = &new_data.content {
        let lock = ENCRYPTION_KEYS.lock().await;
        if let Some(key) = lock.get(&guild_id) {
            encrypt(key, plaintext).or_else(|| Some(plaintext.as_bytes().to_vec()))
        } else {
            Some(plaintext.as_bytes().to_vec())
        }
    } else {
        None
    };

    let message_id = new_data.message_id.map(|v| v as i64);
    let channel_id = new_data.channel_id.map(|v| v as i64);
    let author_id = new_data.author_id.map(|v| v as i64);

    match sqlx::query!(
        "INSERT INTO action_refs
             (action_id, ref_message_id, ref_channel_id, ref_author_id, ref_content, image_url)
         VALUES ($1, $2, $3, $4, $5, $6)
         ON CONFLICT (action_id) DO UPDATE
             SET ref_message_id = EXCLUDED.ref_message_id,
                 ref_channel_id = EXCLUDED.ref_channel_id,
                 ref_author_id  = EXCLUDED.ref_author_id,
                 ref_content    = EXCLUDED.ref_content,
                 image_url      = EXCLUDED.image_url",
        action_id,
        message_id,
        channel_id,
        author_id,
        content_bytes.as_deref(),
        new_data.image_url.as_deref(),
    )
    .execute(&*SQL)
    .await
    {
        Ok(_) => true,
        Err(err) => {
            warn!("update_ref: failed; err = {err:?}");
            false
        }
    }
}

pub fn embeds_for_ref(ref_data: &RefData) -> Vec<serenity::all::CreateEmbed> {
    if ref_data.is_empty() {
        return vec![];
    }

    let mut description = String::from("**REFERENCE**");

    if let Some(header) = ref_data.header() {
        description.push('\n');
        description.push_str(&header);
    }

    if let Some(content) = &ref_data.content {
        description.push_str(&format!("\n```\n{content}\n```"));
    }

    vec![
        serenity::all::CreateEmbed::new()
            .color(crate::constants::BRAND_BLUE)
            .description(description),
    ]
}

pub fn apply_ref_button(
    msg: serenity::all::CreateMessage,
    db_id: &str,
    ref_data: &RefData,
) -> serenity::all::CreateMessage {
    if !ref_data.is_empty() {
        let action_row = serenity::all::CreateActionRow::Buttons(vec![
            serenity::all::CreateButton::new(format!("view_ref:{}", db_id))
                .label("View Reference")
                .style(serenity::all::ButtonStyle::Secondary),
        ]);
        msg.components(vec![action_row])
    } else {
        msg
    }
}

pub async fn attachments_for_ref(ref_data: &RefData) -> Vec<serenity::all::CreateAttachment> {
    let mut attachments = Vec::new();
    if let Some(urls_str) = &ref_data.image_url {
        for (i, url) in urls_str.split(',').enumerate() {
            if let Ok(req) = reqwest::get(url).await {
                if let Ok(bytes) = req.bytes().await {
                    let ext = url
                        .split('?')
                        .next()
                        .unwrap_or("")
                        .rsplit('.')
                        .next()
                        .unwrap_or("png");
                    attachments.push(serenity::all::CreateAttachment::bytes(
                        bytes.to_vec(),
                        format!("reference_{}.{}", i, ext),
                    ));
                }
            }
        }
    }
    attachments
}

pub async fn upload_attachments(
    guild_id: u64,
    attachments: &[serenity::all::Attachment],
) -> Option<String> {
    let mut image_urls = Vec::new();
    for att in attachments.iter().filter(|a| {
        a.content_type
            .as_deref()
            .unwrap_or("")
            .starts_with("image/")
    }) {
        let url = att.url.clone();
        let filename = att.filename.clone();

        let token = s3::random_token();
        let ext = filename.rsplit('.').next().unwrap_or("bin").to_string();
        let (key, predicted_url) = s3::get_predicted_url(guild_id, &token, &ext);

        image_urls.push(predicted_url);

        tokio::spawn(async move {
            if let Ok(req) = reqwest::get(&url).await {
                if let Ok(bytes) = req.bytes().await {
                    let data = bytes.to_vec();
                    let ct = s3::detect_content_type(&data);
                    let _ = s3::upload_image_with_key(key, data, ct).await;
                }
            }
        });
    }

    if image_urls.is_empty() {
        None
    } else {
        Some(image_urls.join(","))
    }
}
