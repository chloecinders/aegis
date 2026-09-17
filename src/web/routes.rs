use axum::Json;
use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{Html, IntoResponse, Redirect, Response};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::domain::Snowflake;
use crate::features::archive::transcript::{self, store};
use crate::platform::crypto;
use crate::web::Shared;
use crate::web::directory::Entry;
use crate::web::session::{Membership, Session};

pub async fn health() -> &'static str {
    "ok"
}

async fn admitted(
    web: &Shared,
    headers: &HeaderMap,
    guild: Snowflake,
) -> Result<Session, StatusCode> {
    let session = crate::web::dash::auth::viewer(web, headers)
        .await
        .ok_or(StatusCode::UNAUTHORIZED)?;

    match session
        .guilds
        .iter()
        .find(|membership| membership.id == guild)
        .is_some_and(Membership::moderates)
    {
        true => Ok(session),
        false => Err(StatusCode::FORBIDDEN),
    }
}

async fn visible(
    web: &Shared,
    headers: &HeaderMap,
    guild: Snowflake,
    id: &str,
) -> Result<Vec<Snowflake>, StatusCode> {
    let session = admitted(web, headers, guild).await?;

    let covered = store::channels(&web.pool, guild, id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    if covered.is_empty() {
        return Err(StatusCode::NOT_FOUND);
    }

    let allowed = web.guilds.readable(guild, session.user, covered).await;

    match allowed.is_empty() {
        true => Err(StatusCode::FORBIDDEN),
        false => Ok(allowed),
    }
}

pub async fn page(
    State(web): State<Shared>,
    headers: HeaderMap,
    Path((guild, id)): Path<(Snowflake, String)>,
) -> Response {
    if crate::web::dash::auth::viewer(&web, &headers)
        .await
        .is_some()
    {
        return Html(include_str!(concat!(env!("OUT_DIR"), "/transcript.html"))).into_response();
    }

    let plain = !id.is_empty()
        && id
            .chars()
            .all(|letter| letter.is_ascii_alphanumeric() || letter == '-');

    let back = match plain {
        true => format!("{}?next=/transcript/{guild}/{id}", crate::web::SIGN_IN),
        false => String::from(crate::web::SIGN_IN),
    };

    Redirect::to(&back).into_response()
}

#[derive(Debug, Deserialize)]
pub struct Paging {
    pub before: Option<Snowflake>,
    pub limit: Option<i64>,
}

#[derive(Debug, Serialize)]
pub struct Rendered {
    #[serde(with = "crate::web::flat")]
    pub id: Snowflake,
    #[serde(with = "crate::web::flat")]
    pub channel: Snowflake,
    #[serde(with = "crate::web::flat")]
    pub author: Snowflake,
    pub name: String,
    pub display: Option<String>,
    pub avatar: Option<String>,
    #[serde(
        with = "crate::web::flat::maybe",
        skip_serializing_if = "Option::is_none"
    )]
    pub reply_to: Option<Snowflake>,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub reply_collected: bool,
    pub content: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub files: Vec<String>,
    pub at: DateTime<Utc>,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub removed: bool,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub system: bool,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub edits: Vec<Revision>,
}

#[derive(Debug, Serialize)]
pub struct Revision {
    pub content: String,
    pub at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct Answer {
    #[serde(with = "crate::web::flat::maybe")]
    pub next: Option<Snowflake>,
    pub messages: Vec<Rendered>,
}

#[derive(Debug, Serialize)]
pub struct Header {
    #[serde(flatten)]
    pub meta: transcript::Meta,
    pub title: String,
    pub spans_channels: bool,
    pub jumpable: bool,
    pub channels: Vec<Entry>,
}

impl Header {
    fn of(meta: transcript::Meta, channels: Vec<Entry>) -> Self {
        Self {
            title: meta.title(),
            spans_channels: meta.scope.spans_channels(),
            jumpable: matches!(meta.scope, transcript::Scope::User),
            channels,
            meta,
        }
    }
}

pub async fn meta(
    State(web): State<Shared>,
    headers: HeaderMap,
    Path((guild, id)): Path<(Snowflake, String)>,
) -> Result<Json<Header>, StatusCode> {
    let allowed = visible(&web, &headers, guild, &id).await?;

    let found = match store::meta(&web.pool, guild, &id, &allowed).await {
        Ok(Some(found)) => found,
        Ok(None) => return Err(StatusCode::NOT_FOUND),
        Err(_) => return Err(StatusCode::INTERNAL_SERVER_ERROR),
    };

    let channels = match found.scope.spans_channels() {
        true => visited(&web, guild, &allowed).await,
        false => Vec::new(),
    };

    Ok(Json(Header::of(found, channels)))
}

async fn visited(web: &Shared, guild: Snowflake, allowed: &[Snowflake]) -> Vec<Entry> {
    let Some(view) = web.guilds.view(guild).await else {
        return Vec::new();
    };

    allowed
        .iter()
        .filter_map(|channel| view.channels.iter().find(|listed| listed.id == *channel))
        .cloned()
        .collect()
}

pub async fn messages(
    State(web): State<Shared>,
    headers: HeaderMap,
    Path((guild, id)): Path<(Snowflake, String)>,
    Query(paging): Query<Paging>,
) -> Result<impl IntoResponse, StatusCode> {
    let allowed = visible(&web, &headers, guild, &id).await?;

    let limit = transcript::limit(paging.limit);
    let page = store::page(&web.pool, &id, paging.before, limit, &allowed)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let key = web
        .keys
        .key(guild)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let messages: Vec<Rendered> = page
        .messages
        .into_iter()
        .map(|stored| Rendered {
            id: stored.message,
            channel: stored.channel,
            author: stored.author,
            name: stored.author_name,
            display: stored.author_display_name,
            avatar: stored.author_avatar_url,
            reply_to: stored.referenced,
            reply_collected: stored.referenced_collected,
            content: open(key.as_ref(), stored.content.as_deref()),
            files: stored.attachments.map(links).unwrap_or_default(),
            at: stored.created_at,
            removed: stored.removed,
            system: stored.system,
            edits: stored
                .edits
                .into_iter()
                .map(|edit| Revision {
                    content: open(key.as_ref(), edit.content.as_deref()),
                    at: edit.at,
                })
                .collect(),
        })
        .collect();

    Ok(Json(Answer {
        next: page.next,
        messages,
    }))
}

fn links(stored: serde_json::Value) -> Vec<String> {
    let serde_json::Value::Array(listed) = stored else {
        return Vec::new();
    };

    listed
        .into_iter()
        .filter_map(|entry| match entry {
            serde_json::Value::String(url) => Some(url),
            serde_json::Value::Object(mut fields) => match fields.remove("url") {
                Some(serde_json::Value::String(url)) => Some(url),
                _ => None,
            },
            _ => None,
        })
        .collect()
}

fn open(key: Option<&crypto::Secret>, stored: Option<&[u8]>) -> String {
    let Some(stored) = stored else {
        return String::new();
    };

    let opened = match key {
        Some(key) => crypto::decrypt(key, stored),
        None => String::from_utf8(stored.to_vec()).ok(),
    };

    opened.unwrap_or_else(|| String::from("[unreadable]"))
}
