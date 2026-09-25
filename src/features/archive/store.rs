use serenity::all;
use sqlx::PgPool;

use crate::command::error::{Ctx, Result};
use crate::domain::Snowflake;
use crate::features::archive::Storable;
use crate::features::settings::store;
use crate::platform::db::writer::Batched;

pub async fn keep(pool: &PgPool, batch: &[Storable]) -> Result<()> {
    let ids: Vec<i64> = batch
        .iter()
        .map(|storable| storable.message.id as i64)
        .collect();
    let channels: Vec<i64> = batch
        .iter()
        .map(|storable| storable.message.channel_id as i64)
        .collect();
    let guilds: Vec<i64> = batch
        .iter()
        .map(|storable| storable.message.guild_id.unwrap_or_default() as i64)
        .collect();
    let authors: Vec<i64> = batch
        .iter()
        .map(|storable| storable.message.author.id as i64)
        .collect();
    let names: Vec<String> = batch
        .iter()
        .map(|storable| storable.message.author.name.clone())
        .collect();
    let display: Vec<Option<String>> = batch
        .iter()
        .map(|storable| storable.message.author.display_name.clone())
        .collect();
    let avatars: Vec<Option<String>> = batch
        .iter()
        .map(|storable| storable.message.author.avatar_url.clone())
        .collect();
    let parents: Vec<Option<i64>> = batch
        .iter()
        .map(|storable| storable.message.referenced_message_id.map(|id| id as i64))
        .collect();
    let bodies: Vec<Option<Vec<u8>>> = batch.iter().map(|storable| storable.body.clone()).collect();
    let attachments: Vec<serde_json::Value> = batch
        .iter()
        .map(|storable| serde_json::to_value(&storable.message.attachments).unwrap_or_default())
        .collect();
    let stamps: Vec<chrono::DateTime<chrono::Utc>> = batch
        .iter()
        .map(|storable| storable.message.created_at)
        .collect();
    let systems: Vec<bool> = batch.iter().map(|storable| storable.system).collect();

    sqlx::query!(
        "INSERT INTO messages (message_id, channel_id, guild_id, author_id, author_name,
            author_display_name, author_avatar_url, referenced_message_id, content,
            attachment_urls, created_at, system)
        SELECT * FROM UNNEST($1::bigint[], $2::bigint[], $3::bigint[], $4::bigint[], $5::text[],
            $6::text[], $7::text[], $8::bigint[], $9::bytea[], $10::jsonb[], $11::timestamptz[],
            $12::bool[])
        ON CONFLICT DO NOTHING",
        &ids,
        &channels,
        &guilds,
        &authors,
        &names,
        &display as &[Option<String>],
        &avatars as &[Option<String>],
        &parents as &[Option<i64>],
        &bodies as &[Option<Vec<u8>>],
        &attachments,
        &stamps,
        &systems
    )
    .execute(pool)
    .await
    .ctx("store messages")?;

    Ok(())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Removal {
    Manual,
    Automod,
}

impl Removal {
    pub fn as_str(&self) -> &'static str {
        match self {
            Removal::Manual => "manual",
            Removal::Automod => "automod",
        }
    }
}

pub async fn removed(
    pool: &PgPool,
    guild: Snowflake,
    message: Snowflake,
    source: Removal,
    rule: Option<&str>,
) -> Result<()> {
    sqlx::query!(
        "INSERT INTO message_deletions (message_id, guild_id, source, rule)
        VALUES ($1, $2, $3::message_removal, $4) ON CONFLICT DO NOTHING",
        message as i64,
        guild as i64,
        source.as_str() as _,
        rule
    )
    .execute(pool)
    .await
    .ctx("record a deleted message")?;

    Ok(())
}

pub async fn removed_many(pool: &PgPool, guild: Snowflake, messages: &[Snowflake]) -> Result<()> {
    let ids: Vec<i64> = messages.iter().map(|id| *id as i64).collect();

    sqlx::query!(
        "INSERT INTO message_deletions (message_id, guild_id, source)
        SELECT id, $2, 'manual'::message_removal FROM UNNEST($1::bigint[]) AS id
        ON CONFLICT DO NOTHING",
        &ids,
        guild as i64
    )
    .execute(pool)
    .await
    .ctx("record deleted messages")?;

    Ok(())
}

pub async fn removed_since(
    pool: &PgPool,
    guild: Snowflake,
    author: Snowflake,
    since: chrono::DateTime<chrono::Utc>,
) -> Result<()> {
    sqlx::query!(
        "INSERT INTO message_deletions (message_id, guild_id, source)
        SELECT message_id, $1, 'manual'::message_removal FROM messages
        WHERE guild_id = $1 AND author_id = $2 AND created_at >= $3
        ON CONFLICT DO NOTHING",
        guild as i64,
        author as i64,
        since
    )
    .execute(pool)
    .await
    .ctx("record cleared messages")?;

    Ok(())
}

pub struct Revision {
    pub message: Snowflake,
    pub body: Option<Vec<u8>>,
    pub at: chrono::DateTime<chrono::Utc>,
}

pub async fn revise(pool: &PgPool, revision: &Revision) -> Result<()> {
    sqlx::query!(
        "INSERT INTO message_edits (message_id, content, created_at) VALUES ($1, $2, $3)",
        revision.message as i64,
        revision.body.as_deref(),
        revision.at
    )
    .execute(pool)
    .await
    .ctx("store message edit")?;

    Ok(())
}

pub fn sink(pool: PgPool) -> Batched<Storable> {
    Batched::spawn("message", move |batch| {
        let pool = pool.clone();

        async move {
            if let Err(failure) = keep(&pool, &batch).await {
                tracing::warn!("could not store messages; err = {failure}");
            }
        }
    })
}

pub async fn enable(
    pool: &PgPool,
    guild: Snowflake,
    channel: all::ChannelId,
    message: all::MessageId,
) -> Result<()> {
    store::enroll(pool, guild).await?;

    sqlx::query!(
        "INSERT INTO guild_encryption (guild_id, enabled, key_channel_id, key_message_id)
        VALUES ($1, true, $2, $3)
        ON CONFLICT (guild_id) DO UPDATE SET enabled = true,
            key_channel_id = EXCLUDED.key_channel_id, key_message_id = EXCLUDED.key_message_id",
        guild as i64,
        channel.get() as i64,
        message.get() as i64
    )
    .execute(pool)
    .await
    .ctx("enable encryption")?;

    Ok(())
}

pub async fn disable(pool: &PgPool, guild: Snowflake) -> Result<u64> {
    sqlx::query!(
        "UPDATE guild_encryption SET enabled = false, key_channel_id = NULL,
        key_message_id = NULL WHERE guild_id = $1",
        guild as i64
    )
    .execute(pool)
    .await
    .ctx("disable encryption")?;

    let wiped = sqlx::query!("DELETE FROM messages WHERE guild_id = $1", guild as i64)
        .execute(pool)
        .await
        .ctx("erase stored messages")?;

    Ok(wiped.rows_affected())
}

pub struct DeletedMessage {
    pub message: Snowflake,
    pub author: Snowflake,
    pub author_name: String,
    pub author_display_name: Option<String>,
    pub author_avatar_url: Option<String>,
    pub parent: Option<Snowflake>,
    pub body: Option<Vec<u8>>,
    pub attachments: Option<serde_json::Value>,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

pub async fn count_deleted(pool: &PgPool, guild: Snowflake, channel: Snowflake) -> Result<u64> {
    let counted = sqlx::query_scalar!(
        r#"SELECT count(*) AS "count!" FROM message_deletions d
        JOIN messages m ON m.message_id = d.message_id
        WHERE d.guild_id = $1 AND m.channel_id = $2 AND d.source = 'manual' AND NOT m.system"#,
        guild as i64,
        channel as i64
    )
    .fetch_one(pool)
    .await
    .ctx("count deleted messages")?;

    Ok(counted as u64)
}

pub async fn nth_deleted(
    pool: &PgPool,
    guild: Snowflake,
    channel: Snowflake,
    at: u64,
) -> Result<Option<DeletedMessage>> {
    let row = sqlx::query!(
        "SELECT m.message_id, m.author_id, m.author_name, m.author_display_name,
            m.author_avatar_url, m.referenced_message_id, m.content, m.attachment_urls,
            m.created_at
        FROM message_deletions d
        JOIN messages m ON m.message_id = d.message_id
        WHERE d.guild_id = $1 AND m.channel_id = $2 AND d.source = 'manual' AND NOT m.system
        ORDER BY d.deleted_at DESC, d.message_id DESC
        OFFSET $3 LIMIT 1",
        guild as i64,
        channel as i64,
        at as i64
    )
    .fetch_optional(pool)
    .await
    .ctx("read a deleted message")?;

    Ok(row.map(|row| DeletedMessage {
        message: row.message_id as Snowflake,
        author: row.author_id as Snowflake,
        author_name: row.author_name,
        author_display_name: row.author_display_name,
        author_avatar_url: row.author_avatar_url,
        parent: row.referenced_message_id.map(|id| id as Snowflake),
        body: row.content,
        attachments: row.attachment_urls,
        created_at: row.created_at,
    }))
}
