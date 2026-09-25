use axum::Json;
use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, header};
use axum::response::{IntoResponse, Redirect, Response};
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};

use crate::domain::Snowflake;
use crate::web::dash::rejection::Rejection;
use crate::web::directory::View;
use crate::web::session::{self, Membership, Session};
use crate::web::{Shared, oauth};

pub async fn viewer(web: &Shared, headers: &HeaderMap) -> Option<Session> {
    let cookie = headers
        .get(header::COOKIE)
        .and_then(|cookie| cookie.to_str().ok());

    let carried = headers
        .get(header::AUTHORIZATION)
        .and_then(|header| header.to_str().ok())
        .and_then(|header| header.strip_prefix("Bearer "))
        .map(str::trim);

    let token = carried.or_else(|| session::from_cookies(cookie))?;

    web.sessions.read(token).await
}

pub async fn signed(web: &Shared, headers: &HeaderMap) -> Result<Session, Rejection> {
    viewer(web, headers).await.ok_or(Rejection::unauthorized())
}

pub async fn administers(
    web: &Shared,
    headers: &HeaderMap,
    guild: Snowflake,
) -> Result<Membership, Rejection> {
    signed(web, headers)
        .await?
        .guilds
        .into_iter()
        .find(|membership| membership.id == guild)
        .filter(Membership::administers)
        .ok_or(Rejection::forbidden())
}

pub fn authors(web: &Shared, session: &Session) -> bool {
    web.developers.contains(&session.user)
}

pub async fn authoring_viewer(web: &Shared, headers: &HeaderMap) -> Result<Session, Rejection> {
    let session = signed(web, headers).await?;

    match authors(web, &session) {
        true => Ok(session),
        false => Err(Rejection::forbidden()),
    }
}

#[derive(Debug, Deserialize)]
pub struct Onward {
    pub next: Option<String>,
}

fn landing(asked: Option<&str>) -> String {
    let wanted = asked.unwrap_or("/dashboard");

    match wanted.starts_with('/') && !wanted.starts_with("//") {
        true => wanted.to_string(),
        false => String::from("/dashboard"),
    }
}

pub async fn sign_in(State(web): State<Shared>, Query(onward): Query<Onward>) -> Response {
    let Some(oauth) = web.oauth.as_ref() else {
        return Rejection::missing().into_response();
    };

    let state = web.sessions.begin(&landing(onward.next.as_deref()));

    Redirect::temporary(&oauth.authorize(&state)).into_response()
}

#[derive(Debug, Deserialize)]
pub struct Returned {
    pub code: Option<String>,
    pub state: Option<String>,
}

pub async fn callback(State(web): State<Shared>, Query(back): Query<Returned>) -> Response {
    let Some(oauth) = web.oauth.as_ref() else {
        return Rejection::missing().into_response();
    };

    let (Some(code), Some(state)) = (back.code, back.state) else {
        return Redirect::temporary("/dashboard").into_response();
    };

    let Some(destination) = web.sessions.finish(&state) else {
        return Redirect::temporary("/dashboard").into_response();
    };

    let identified = oauth.identify(&web.client, &code, true).await;

    let Ok((profile, guilds)) = identified else {
        return Redirect::temporary("/dashboard").into_response();
    };

    let expires = Utc::now() + Duration::hours(12);

    let Some(opened) = oauth::opened(profile, guilds, expires) else {
        return Redirect::temporary("/dashboard").into_response();
    };

    let token = web.sessions.open(opened).await;

    (
        [(header::SET_COOKIE, session::handed(&token, &web.site))],
        Redirect::temporary(&destination),
    )
        .into_response()
}

pub async fn sign_out(State(web): State<Shared>, headers: HeaderMap) -> Response {
    let cookie = headers
        .get(header::COOKIE)
        .and_then(|cookie| cookie.to_str().ok());

    if let Some(token) = session::from_cookies(cookie) {
        web.sessions.close(token).await;
    }

    (
        [(header::SET_COOKIE, session::cleared(&web.site))],
        Redirect::temporary("/"),
    )
        .into_response()
}

#[derive(Debug, Serialize)]
pub struct Identity {
    #[serde(with = "crate::web::flat")]
    pub user: Snowflake,
    pub name: String,
    pub display: Option<String>,
    pub avatar: Option<String>,
    pub manages: Vec<Membership>,
    pub developer: bool,
}

pub async fn identity(
    State(web): State<Shared>,
    headers: HeaderMap,
) -> Result<Json<Identity>, Rejection> {
    let session = signed(&web, &headers).await?;

    let manages = session
        .guilds
        .iter()
        .filter(|membership| membership.administers() && web.guilds.present(membership.id))
        .cloned()
        .collect();

    Ok(Json(Identity {
        developer: authors(&web, &session),
        user: session.user,
        name: session.name,
        display: session.display,
        avatar: session.avatar,
        manages,
    }))
}

pub async fn guild(
    State(web): State<Shared>,
    headers: HeaderMap,
    Path(guild): Path<Snowflake>,
) -> Result<Json<View>, Rejection> {
    administers(&web, &headers, guild).await?;

    web.guilds
        .view(guild)
        .await
        .map(Json)
        .ok_or(Rejection::missing())
}

#[derive(Debug, Deserialize)]
pub struct Handed {
    pub code: String,
}

#[derive(Debug, Serialize)]
pub struct Opened {
    pub token: String,
    pub expires: DateTime<Utc>,
}

pub async fn activity(
    State(web): State<Shared>,
    Json(handed): Json<Handed>,
) -> Result<Json<Opened>, Rejection> {
    let oauth = web.oauth.as_ref().ok_or(Rejection::missing())?;

    let (profile, guilds) = oauth
        .identify(&web.client, &handed.code, false)
        .await
        .map_err(|_| Rejection::upstream())?;

    let expires = Utc::now() + Duration::hours(12);

    let opened = oauth::opened(profile, guilds, expires).ok_or(Rejection::upstream())?;

    Ok(Json(Opened {
        token: web.sessions.open(opened).await,
        expires,
    }))
}
