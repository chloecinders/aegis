use regex::Regex;
use serenity::all::{ChannelId, Context, Mentionable, Message, MessageId};
use tracing::warn;

use crate::{SQL, utils::s3};

fn extract_message_content(msg: &Message) -> String {
    if !msg.content.is_empty() {
        msg.content.clone()
    } else if let Some(embed) = msg.embeds.first() {
        let desc = embed.description.clone().unwrap_or_default();

        if embed.kind.clone().unwrap_or_default() == "auto_moderation_message" {
            format!("Automod: {}", desc)
        } else if desc.starts_with("**MESSAGE DELETED**") || desc.starts_with("**MESSAGE EDITED**")
        {
            String::from("<Stored context lost>")
        } else {
            String::new()
        }
    } else {
        String::new()
    }
}

pub async fn resolve_ref(
    ctx: &Context,
    msg: &Message,
    _action_id: &str,
    explicit_ref_url: Option<&str>,
) -> (Option<String>, Option<String>) {
    let guild_id = msg.guild_id.map(|g| g.get()).unwrap_or(0);

    let url = match explicit_ref_url {
        Some(u) => u,
        None => {
            let target_msg = msg.referenced_message.as_deref().unwrap_or(msg);

            let content = if std::ptr::eq(target_msg, msg) {
                None
            } else {
                let mut c = format!(
                    "-# ID: `{}` [Jump]({}) | Author: {}\n",
                    target_msg.id.get(),
                    target_msg.link(),
                    target_msg.author.mention()
                );

                let extracted = extract_message_content(target_msg);

                if !extracted.is_empty() {
                    c.push_str(&format!("```\n{}\n```", extracted));
                }

                Some(c)
            };

            let image_url = upload_attachments(guild_id, &target_msg.attachments).await;

            return (content, image_url);
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
                let mut c = format!(
                    "-# ID: `{}` [Jump]({}) | Author: {}\n",
                    fetched.id.get(),
                    fetched.link(),
                    fetched.author.mention()
                );
                let extracted = extract_message_content(&fetched);

                if !extracted.is_empty() {
                    c.push_str(&format!("```\n{}\n```", extracted));
                }

                let content = Some(c);
                let image_url = upload_attachments(guild_id, &fetched.attachments).await;

                return (content, image_url);
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

        return (None, Some(predicted_url));
    }

    (Some(format!("```\n{}\n```", url)), None)
}

pub async fn save_ref(
    action_id: &str,
    ref_data: &(Option<String>, Option<String>),
    reason_is_default: bool,
) {
    let (message_content, image_url) = ref_data;

    if reason_is_default {
        if let Some(content) = message_content {
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

    if message_content.is_none() && image_url.is_none() {
        return;
    }

    if let Err(err) = sqlx::query!(
        "INSERT INTO action_refs (action_id, message_content, image_url)
         VALUES ($1, $2, $3)
         ON CONFLICT (action_id) DO UPDATE
             SET message_content = COALESCE(EXCLUDED.message_content, action_refs.message_content),
                 image_url       = COALESCE(EXCLUDED.image_url,       action_refs.image_url)",
        action_id,
        message_content.as_deref(),
        image_url.as_deref(),
    )
    .execute(&*SQL)
    .await
    {
        warn!("save_ref: failed to save action_refs; err = {err:?}");
    }
}

pub fn embeds_for_ref(
    ref_data: &(Option<String>, Option<String>),
) -> Vec<serenity::all::CreateEmbed> {
    if ref_data.0.is_none() && ref_data.1.is_none() {
        return vec![];
    }

    let mut embeds = Vec::new();
    let mut main_embed = serenity::all::CreateEmbed::new().color(crate::constants::BRAND_BLUE);

    if let Some(content) = &ref_data.0 {
        main_embed = main_embed.description(format!("**REFERENCE**\n{content}"));
    } else {
        main_embed = main_embed.description("**REFERENCE**");
    }

    embeds.push(main_embed);

    embeds
}

pub async fn attachments_for_ref(
    ref_data: &(Option<String>, Option<String>),
) -> Vec<serenity::all::CreateAttachment> {
    let mut attachments = Vec::new();
    if let Some(urls_str) = &ref_data.1 {
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

pub async fn get_ref(action_id: &str) -> Option<(Option<String>, Option<String>)> {
    let row = sqlx::query!(
        "SELECT message_content, image_url FROM action_refs WHERE action_id = $1",
        action_id
    )
    .fetch_optional(&*SQL)
    .await
    .ok()??;

    Some((row.message_content, row.image_url))
}

pub async fn update_ref(ctx: &Context, action_id: &str, new_ref_url: &str) -> bool {
    let guild_id = sqlx::query!("SELECT guild_id FROM actions WHERE id = $1", action_id)
        .fetch_optional(&*SQL)
        .await
        .ok()
        .flatten()
        .map(|r| r.guild_id as u64)
        .unwrap_or(0);

    let (message_content, image_url) = {
        let mut result = (Some(new_ref_url.to_string()), None);

        let re = Regex::new(
            r"https?://(?:canary\.|ptb\.)?discord(?:app)?\.com/channels/(\d+)/(\d+)/(\d+)",
        )
        .unwrap();
        if let Some(caps) = re.captures(new_ref_url) {
            if let (Ok(channel_id), Ok(message_id)) =
                (caps[2].parse::<u64>(), caps[3].parse::<u64>())
            {
                if let Ok(fetched) = ChannelId::new(channel_id)
                    .message(ctx, MessageId::new(message_id))
                    .await
                {
                    let mut c = format!(
                        "-# ID: `{}` [Jump]({}) | Author: {}\n",
                        fetched.id.get(),
                        fetched.link(),
                        fetched.author.mention()
                    );
                    let extracted = extract_message_content(&fetched);

                    if !extracted.is_empty() {
                        c.push_str(&format!("```\n{}\n```", extracted));
                    }

                    let content = Some(c);
                    let img_url = upload_attachments(guild_id, &fetched.attachments).await;

                    result = (content, img_url);
                }
            }
        } else {
            let without_query = new_ref_url
                .split('?')
                .next()
                .unwrap_or(new_ref_url)
                .to_lowercase();
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

                let url_clone = new_ref_url.to_string();
                tokio::spawn(async move {
                    if let Ok(req) = reqwest::get(&url_clone).await {
                        if let Ok(bytes) = req.bytes().await {
                            let data = bytes.to_vec();
                            let ct = s3::detect_content_type(&data);
                            let _ = s3::upload_image_with_key(key, data, ct).await;
                        }
                    }
                });

                result = (None, Some(predicted_url));
            } else {
                result = (Some(format!("```\n{}\n```", new_ref_url)), None);
            }
        }
        result
    };

    if message_content.is_none() && image_url.is_none() {
        return false;
    }

    match sqlx::query!(
        "INSERT INTO action_refs (action_id, message_content, image_url)
         VALUES ($1, $2, $3)
         ON CONFLICT (action_id) DO UPDATE
             SET message_content = EXCLUDED.message_content,
                 image_url       = EXCLUDED.image_url",
        action_id,
        message_content,
        image_url,
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

pub fn apply_ref_button(
    msg: serenity::all::CreateMessage,
    db_id: &str,
    ref_data: &(Option<String>, Option<String>),
) -> serenity::all::CreateMessage {
    if !embeds_for_ref(ref_data).is_empty() {
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
