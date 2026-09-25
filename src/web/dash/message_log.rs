use axum::Json;
use axum::extract::{Path, State};
use axum::http::HeaderMap;
use serde::{Deserialize, Serialize};

use crate::domain::Snowflake;
use crate::domain::ids::TranscriptId;
use crate::features::archive::store as archive;
use crate::features::archive::transcript::{Request, store};
use crate::web::Shared;
use crate::web::dash::auth::administers;
use crate::web::dash::rejection::Rejection;

#[derive(Debug, Deserialize)]
pub struct Asked {
    pub user: String,
}

#[derive(Debug, Serialize)]
pub struct Built {
    pub id: TranscriptId,
}

pub async fn build(
    State(web): State<Shared>,
    headers: HeaderMap,
    Path(guild): Path<Snowflake>,
    Json(asked): Json<Asked>,
) -> Result<Json<Built>, Rejection> {
    let membership = administers(&web, &headers, guild).await?;

    let user = asked
        .user
        .trim()
        .parse::<Snowflake>()
        .ok()
        .filter(|id| *id != 0)
        .ok_or_else(|| Rejection::unusable("expected a user id"))?;

    let Some(name) = archive::author_name(&web.pool, guild, user).await? else {
        return Err(Rejection::unusable("no stored messages found"));
    };

    let request = Request::history(guild, user, name, membership.name);

    let Some(id) = store::build(&web.pool, &request).await? else {
        return Err(Rejection::unusable("no stored messages found"));
    };

    Ok(Json(Built { id }))
}
