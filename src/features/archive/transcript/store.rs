use std::collections::HashMap;

use chrono::{DateTime, Utc};
use sqlx::PgPool;

use crate::command::error::{Ctx, Result};
use crate::domain::Snowflake;
use crate::domain::ids::TranscriptId;
use crate::features::archive::transcript::{Meta, Page, Request, Scope};

pub async fn build(pool: &PgPool, asked: &Request) -> Result<Option<TranscriptId>> {
    if !asked.is_answerable() {
        return Ok(None);
    }

    let id = TranscriptId::generate();
    let mut tx = pool.begin().await.ctx("open transcript")?;

    sqlx::query!(
        "INSERT INTO transcripts
            (transcript_id, guild_id, scope, channel_id, channel_name, subject_id,
            subject_name, window_start, window_end, moderator_name)
        VALUES ($1, $2, $3::transcript_scope, $4, $5, $6, $7, $8, $9, $10)",
        id.as_str(),
        asked.guild as i64,
        asked.scope.as_str() as _,
        asked.channel.map(|id| id as i64),
        asked.channel_name,
        asked.subject.map(|id| id as i64),
        asked.subject_name,
        asked.window_start,
        asked.window_end,
        asked.moderator_name
    )
    .execute(&mut *tx)
    .await
    .ctx("open transcript")?;

    let selected: Vec<i64> = asked.selected.iter().map(|id| *id as i64).collect();

    let collected = sqlx::query!(
        "INSERT INTO transcript_messages (transcript_id, message_id, created_at)
        SELECT $1, message_id, created_at FROM messages
        WHERE guild_id = $2
            AND ($3::bigint IS NULL OR channel_id = $3)
            AND ($4::bigint IS NULL OR author_id = $4)
            AND ($5::timestamptz IS NULL OR created_at >= $5)
            AND ($6::timestamptz IS NULL OR created_at <= $6)
            AND ($7::bigint[] IS NULL OR message_id = ANY ($7))
            AND (NOT $8::bool OR EXISTS (SELECT 1 FROM message_deletions d
                WHERE d.message_id = messages.message_id))
        ON CONFLICT DO NOTHING",
        id.as_str(),
        asked.guild as i64,
        asked.channel.map(|id| id as i64),
        asked.subject.map(|id| id as i64),
        asked.window_start,
        asked.window_end,
        match asked.scope {
            Scope::Selection => Some(&selected[..]),
            _ => None,
        },
        matches!(asked.scope, Scope::Cleared)
    )
    .execute(&mut *tx)
    .await
    .ctx("collect transcript")?;

    if collected.rows_affected() == 0 {
        tx.rollback().await.ctx("discard empty transcript")?;

        return Ok(None);
    }

    tx.commit().await.ctx("save transcript")?;

    Ok(Some(id))
}

pub async fn meta(
    pool: &PgPool,
    guild: Snowflake,
    id: &str,
    visible: &[Snowflake],
) -> Result<Option<Meta>> {
    let visible: Vec<i64> = visible.iter().map(|id| *id as i64).collect();

    let row = sqlx::query!(
        r#"SELECT t.transcript_id, t.guild_id, t.scope::text AS "scope!", t.channel_id,
            t.channel_name, t.subject_id, t.subject_name, t.window_start, t.window_end,
            t.moderator_name, t.created_at,
            (SELECT count(*) FROM transcript_messages m
        JOIN messages g ON g.message_id = m.message_id AND g.created_at = m.created_at
        WHERE m.transcript_id = t.transcript_id AND g.channel_id = ANY ($3)) AS "total!"
        FROM transcripts t WHERE t.transcript_id = $1 AND t.guild_id = $2"#,
        id,
        guild as i64,
        &visible
    )
    .fetch_optional(pool)
    .await
    .ctx("read transcript")?;

    Ok(row.and_then(|row| {
        Some(Meta {
            id: TranscriptId::from(row.transcript_id),
            guild: row.guild_id as Snowflake,
            scope: Scope::parse(&row.scope)?,
            channel: row.channel_id.map(|id| id as Snowflake),
            channel_name: row.channel_name,
            subject: row.subject_id.map(|id| id as Snowflake),
            subject_name: row.subject_name,
            window_start: row.window_start,
            window_end: row.window_end,
            moderator_name: row.moderator_name,
            created_at: row.created_at,
            total: row.total,
        })
    }))
}

pub struct Edit {
    pub content: Option<Vec<u8>>,
    pub at: DateTime<Utc>,
}

pub struct Stored {
    pub message: Snowflake,
    pub channel: Snowflake,
    pub author: Snowflake,
    pub author_name: String,
    pub author_display_name: Option<String>,
    pub author_avatar_url: Option<String>,
    pub referenced: Option<Snowflake>,
    pub referenced_collected: bool,
    pub content: Option<Vec<u8>>,
    pub attachments: Option<serde_json::Value>,
    pub created_at: DateTime<Utc>,
    pub removed: bool,
    pub system: bool,
    pub edits: Vec<Edit>,
}

pub async fn channels(pool: &PgPool, guild: Snowflake, id: &str) -> Result<Vec<Snowflake>> {
    let rows = sqlx::query!(
        "SELECT m.channel_id, min(m.message_id) AS \"first!\"
        FROM transcript_messages t
        JOIN transcripts s ON s.transcript_id = t.transcript_id
        JOIN messages m ON m.message_id = t.message_id AND m.created_at = t.created_at
        WHERE t.transcript_id = $1 AND s.guild_id = $2
        GROUP BY m.channel_id
        ORDER BY \"first!\"",
        id,
        guild as i64
    )
    .fetch_all(pool)
    .await
    .ctx("read transcript channels")?;

    Ok(rows
        .into_iter()
        .map(|row| row.channel_id as Snowflake)
        .collect())
}

pub async fn page(
    pool: &PgPool,
    id: &str,
    before: Option<Snowflake>,
    limit: i64,
    visible: &[Snowflake],
) -> Result<Page<Stored>> {
    let visible: Vec<i64> = visible.iter().map(|id| *id as i64).collect();

    let rows = sqlx::query!(
        "SELECT m.message_id, m.channel_id, m.author_id, m.author_name,
            m.author_display_name, m.author_avatar_url, m.referenced_message_id,
            m.content, m.attachment_urls, m.created_at, m.system,
            (d.message_id IS NOT NULL) AS \"removed!\",
            (r.message_id IS NOT NULL) AS \"referenced_collected!\"
        FROM transcript_messages t
        JOIN messages m ON m.message_id = t.message_id AND m.created_at = t.created_at
        LEFT JOIN message_deletions d ON d.message_id = m.message_id
        LEFT JOIN transcript_messages p ON p.transcript_id = t.transcript_id
            AND p.message_id = m.referenced_message_id
        LEFT JOIN messages r ON r.message_id = p.message_id AND r.created_at = p.created_at
            AND r.channel_id = ANY ($4)
        WHERE t.transcript_id = $1 AND m.channel_id = ANY ($4)
            AND ($2::bigint IS NULL OR m.message_id < $2)
        ORDER BY m.message_id DESC LIMIT $3",
        id,
        before.map(|id| id as i64),
        limit,
        &visible
    )
    .fetch_all(pool)
    .await
    .ctx("read transcript page")?;

    let mut edits = revisions(
        pool,
        &rows.iter().map(|row| row.message_id).collect::<Vec<_>>(),
    )
    .await?;

    let messages: Vec<Stored> = rows
        .into_iter()
        .rev()
        .map(|row| Stored {
            message: row.message_id as Snowflake,
            channel: row.channel_id as Snowflake,
            author: row.author_id as Snowflake,
            author_name: row.author_name,
            author_display_name: row.author_display_name,
            author_avatar_url: row.author_avatar_url,
            referenced: row.referenced_message_id.map(|id| id as Snowflake),
            referenced_collected: row.referenced_collected,
            content: row.content,
            attachments: row.attachment_urls,
            created_at: row.created_at,
            removed: row.removed,
            system: row.system,
            edits: edits.remove(&row.message_id).unwrap_or_default(),
        })
        .collect();

    Ok(Page::of(messages, |message| message.message, limit))
}

async fn revisions(pool: &PgPool, messages: &[i64]) -> Result<HashMap<i64, Vec<Edit>>> {
    let rows = sqlx::query!(
        "SELECT message_id, content, created_at FROM message_edits
        WHERE message_id = ANY ($1)
        ORDER BY message_id, created_at",
        messages
    )
    .fetch_all(pool)
    .await
    .ctx("read transcript edits")?;

    let mut edits: HashMap<i64, Vec<Edit>> = HashMap::new();

    for row in rows {
        edits.entry(row.message_id).or_default().push(Edit {
            content: row.content,
            at: row.created_at,
        });
    }

    Ok(edits)
}
