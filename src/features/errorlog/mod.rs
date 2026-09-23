pub mod store;

use serenity::all::CacheHttp;
use sqlx::PgPool;
use tracing::warn;

use crate::domain::Snowflake;
use crate::domain::logtype::LogType;
use crate::features::settings::store as settings;
use crate::platform::ui::embed::Embed;
use crate::platform::ui::reply;
use crate::platform::ui::tone::Tone;

pub struct Fault {
    pub headline: String,
    pub detail: Option<String>,
}

impl Fault {
    pub fn new(headline: impl Into<String>) -> Self {
        Self {
            headline: headline.into(),
            detail: None,
        }
    }

    pub fn detail(mut self, detail: impl Into<String>) -> Self {
        let detail = detail.into();

        self.detail = match detail.trim().is_empty() {
            true => None,
            false => Some(detail),
        };

        self
    }
}

pub async fn record(pool: &PgPool, http: impl CacheHttp, guild: Snowflake, fault: Fault) {
    let id = match store::keep(pool, guild, &fault).await {
        Ok(id) => id,
        Err(failure) => {
            warn!("could not record an error for guild {guild}; err = {failure}");

            return;
        }
    };

    if !posted(pool, http, guild, &fault).await {
        return;
    }

    if let Err(failure) = store::delivered(pool, id).await {
        warn!("could not mark error {id} delivered; err = {failure}");
    }
}

async fn posted(pool: &PgPool, http: impl CacheHttp, guild: Snowflake, fault: &Fault) -> bool {
    let routed = match settings::routes(pool, guild).await {
        Ok(routed) => routed,
        Err(failure) => {
            warn!("could not read log routes for guild {guild}; err = {failure}");

            return false;
        }
    };

    let Some((_, channel)) = routed
        .into_iter()
        .find(|(kind, _)| *kind == LogType::Errors)
    else {
        return false;
    };

    let stated = Embed::new("ERROR")
        .maybe_lead(Some(fault.headline.clone()))
        .tone(Tone::Danger);

    let entry = match &fault.detail {
        Some(detail) => stated.quote(detail.clone()),
        None => stated,
    };

    let sent = channel
        .widen()
        .send_message(http.http(), reply::plain(&entry))
        .await;

    if let Err(failure) = sent {
        warn!("could not post an error to guild {guild}; err = {failure}");

        return false;
    }

    true
}
