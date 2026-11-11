use std::collections::HashMap;

use serenity::all::{Attachment, CacheHttp, CreateAttachment};
use sha2::{Digest, Sha256};
use sqlx::PgPool;

use crate::app::App;
use crate::app::config::S3;
use crate::command::error::{Ctx, Result};
use crate::domain::Snowflake;
use crate::features::archive::secrets::Encryption;
use crate::features::guildlog::Posted;
use crate::platform::crypto::{self, Secret};
use crate::platform::discord::partial::PartialAttachment;
use crate::platform::s3;
use crate::platform::text;

pub struct Image {
    pub name: String,
    pub bytes: Vec<u8>,
}

#[derive(Default)]
pub struct Recovered {
    pub images: Vec<Image>,
    pub unavailable: Vec<String>,
}

impl Recovered {
    pub fn files(&self) -> Vec<CreateAttachment<'static>> {
        self.images
            .iter()
            .map(|image| CreateAttachment::bytes(image.bytes.clone(), image.name.clone()))
            .collect()
    }
}

pub async fn recover(app: &App, attachments: &[PartialAttachment]) -> Recovered {
    let mut recovered = Recovered::default();
    let mut total = 0;

    let pictures = attachments.iter().filter(|attachment| {
        attachment
            .content_type
            .as_deref()
            .is_some_and(|kind| kind.starts_with("image/"))
    });

    for attachment in pictures.take(10) {
        let bytes = match app.http.bytes(&attachment.url, 8 * 1024 * 1024).await {
            Ok(bytes) if bytes != b"This content is no longer available." => bytes,
            _ => {
                recovered.unavailable.push(attachment.name.clone());

                continue;
            }
        };

        if total + bytes.len() > 24 * 1024 * 1024 {
            break;
        }

        total += bytes.len();
        recovered.images.push(Image {
            name: attachment.name.clone(),
            bytes,
        });
    }

    recovered
}

pub fn scaled(attachment: &Attachment) -> PartialAttachment {
    let dimensions = attachment
        .width
        .zip(attachment.height)
        .map(|(width, height)| (u64::from(width.get()), u64::from(height.get())))
        .filter(|(width, height)| *width.max(height) > 1600);

    let url = match dimensions {
        Some((width, height)) => {
            let longest = width.max(height);
            let joined = match attachment.proxy_url.contains('?') {
                true => '&',
                false => '?',
            };

            format!(
                "{}{joined}width={}&height={}",
                attachment.proxy_url,
                (width * 1600 / longest).max(1),
                (height * 1600 / longest).max(1)
            )
        }
        None => attachment.url.to_string(),
    };

    PartialAttachment {
        name: attachment.filename.to_string(),
        url,
        size: attachment.size,
        content_type: attachment.content_type.as_ref().map(ToString::to_string),
    }
}

async fn upload(
    config: &S3,
    guild: Snowflake,
    key: Option<&Secret>,
    bytes: &[u8],
) -> Option<(String, String)> {
    let encoded = s3::to_webp(bytes)?;

    let body = match key {
        Some(key) => crypto::encrypt(key, &encoded)?,
        None => encoded,
    };

    let token = s3::token();
    let (object, url) = s3::placement(config, "messages", guild, &token);

    s3::store(config, &object, &body)
        .await
        .then_some((token, url))
}

pub async fn preserve(
    app: &App,
    http: impl CacheHttp,
    guild: Snowflake,
    message: Snowflake,
    images: &[Image],
) -> Result<()> {
    let Some(config) = s3::settings(app.config.s3.as_ref()) else {
        return Ok(());
    };

    if images.is_empty() {
        return Ok(());
    }

    let key = match app.secrets.encryption(&app.pool, http, guild).await? {
        Encryption::Off => None,
        Encryption::On(key) => Some(key),
        Encryption::Lost => return Ok(()),
    };

    for image in images {
        let hash = text::hex(&Sha256::digest(&image.bytes));

        let duplicate = sqlx::query!(
            "SELECT token, url FROM message_images
            WHERE guild_id = $1 AND hash = $2 AND encrypted = $3 LIMIT 1",
            guild as i64,
            hash,
            key.is_some()
        )
        .fetch_optional(&app.pool)
        .await
        .ctx("find a stored duplicate image")?;

        let placed = match duplicate {
            Some(row) => Some((row.token, row.url)),
            None => upload(config, guild, key.as_ref(), &image.bytes).await,
        };

        let Some((token, url)) = placed else {
            continue;
        };

        sqlx::query!(
            "INSERT INTO message_images
                (message_id, guild_id, name, token, url, encrypted, hash)
            VALUES ($1, $2, $3, $4, $5, $6, $7)",
            message as i64,
            guild as i64,
            image.name,
            token,
            url,
            key.is_some(),
            hash
        )
        .execute(&app.pool)
        .await
        .ctx("record a stored image")?;
    }

    Ok(())
}

pub async fn restore(
    app: &App,
    http: impl CacheHttp,
    guild: Snowflake,
    message: Snowflake,
    recovered: &mut Recovered,
) -> Result<()> {
    if recovered.unavailable.is_empty() {
        return Ok(());
    }

    let copies = stored(&app.pool, &[message as i64])
        .await?
        .remove(&(message as i64))
        .unwrap_or_default();

    if copies.is_empty() {
        return Ok(());
    }

    let key = app.secrets.of(&app.pool, http, guild).await?;
    let mut missing = Vec::new();

    for name in std::mem::take(&mut recovered.unavailable) {
        let Some(copy) = copies.iter().find(|copy| copy.name == name) else {
            missing.push(name);

            continue;
        };

        let bytes = match (
            app.http.bytes(&copy.url, 8 * 1024 * 1024).await,
            copy.encrypted,
            key.as_ref(),
        ) {
            (Ok(bytes), false, _) => Some(bytes),
            (Ok(ciphertext), true, Some(key)) => crypto::decrypt_bytes(key, &ciphertext),
            _ => None,
        };

        match bytes {
            Some(bytes) => recovered.images.push(Image {
                name: format!("{name}.webp"),
                bytes,
            }),
            None => missing.push(name),
        }
    }

    recovered.unavailable = missing;

    Ok(())
}

pub async fn note_log(pool: &PgPool, message: Snowflake, at: Posted, images: usize) -> Result<()> {
    sqlx::query!(
        "UPDATE message_deletions
        SET log_channel_id = $1, log_message_id = $2, log_images = $3
        WHERE message_id = $4",
        at.channel.get() as i64,
        at.message.get() as i64,
        images as i32,
        message as i64
    )
    .execute(pool)
    .await
    .ctx("record the images on a log entry")?;

    Ok(())
}

pub async fn discard(app: &App, guild: Snowflake, message: Snowflake) -> Result<()> {
    let Some(config) = s3::settings(app.config.s3.as_ref()) else {
        return Ok(());
    };

    let orphaned = sqlx::query_scalar!(
        r#"WITH removed AS (
            DELETE FROM message_images WHERE message_id = $1 RETURNING token
        )
        SELECT DISTINCT removed.token AS "token!" FROM removed
        WHERE NOT EXISTS (
            SELECT 1 FROM message_images other
            WHERE other.token = removed.token AND other.message_id <> $1
        )"#,
        message as i64
    )
    .fetch_all(&app.pool)
    .await
    .ctx("discard stored images")?;

    for token in orphaned {
        s3::remove(config, &s3::placement(config, "messages", guild, &token).0).await;
    }

    Ok(())
}

pub async fn log_urls(
    http: &serenity::http::Http,
    channel: Snowflake,
    message: Snowflake,
) -> Vec<String> {
    let Ok(entry) = http.get_message(channel.into(), message.into()).await else {
        return Vec::new();
    };

    entry
        .attachments
        .iter()
        .filter(|attachment| {
            attachment
                .content_type
                .as_deref()
                .is_some_and(|kind| kind.starts_with("image/"))
        })
        .map(|attachment| attachment.url.to_string())
        .collect()
}

#[derive(Clone, Debug)]
pub struct Stored {
    pub name: String,
    pub token: String,
    pub url: String,
    pub encrypted: bool,
}

pub async fn stored(pool: &PgPool, messages: &[i64]) -> Result<HashMap<i64, Vec<Stored>>> {
    let rows = sqlx::query!(
        "SELECT message_id, name, token, url, encrypted FROM message_images
        WHERE message_id = ANY ($1)
        ORDER BY message_id, created_at",
        messages
    )
    .fetch_all(pool)
    .await
    .ctx("read stored images")?;

    let mut images: HashMap<i64, Vec<Stored>> = HashMap::new();

    for row in rows {
        images.entry(row.message_id).or_default().push(Stored {
            name: row.name,
            token: row.token,
            url: row.url,
            encrypted: row.encrypted,
        });
    }

    Ok(images)
}
