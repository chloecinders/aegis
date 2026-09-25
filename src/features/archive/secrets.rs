use std::time::Duration;

use serenity::all::{CacheHttp, ChannelId, HttpError, MessageId};
use sqlx::PgPool;
use tracing::warn;

use crate::command::error::{Ctx, Result};
use crate::domain::Snowflake;
use crate::features::archive::store;
use crate::features::errorlog;
use crate::platform::cache::Cache;
use crate::platform::crypto::{self, Secret};
use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Encryption {
    Off,
    On(Secret),
    Lost,
}

impl Encryption {
    pub fn key(&self) -> Option<Secret> {
        match self {
            Encryption::On(key) => Some(*key),
            Encryption::Off | Encryption::Lost => None,
        }
    }
}

pub struct Keys {
    known: Cache<Snowflake, Encryption>,
}

impl Default for Keys {
    fn default() -> Self {
        Self::new()
    }
}

impl Keys {
    pub fn new() -> Self {
        Self {
            known: Cache::new(4096, Some(Duration::from_secs(900))),
        }
    }

    pub fn forget(&self, guild: Snowflake) {
        self.known.remove(&guild);
    }

    pub async fn of(
        &self,
        pool: &PgPool,
        http: impl CacheHttp,
        guild: Snowflake,
    ) -> Result<Option<Secret>> {
        Ok(self.encryption(pool, http, guild).await?.key())
    }

    pub async fn encryption(
        &self,
        pool: &PgPool,
        http: impl CacheHttp,
        guild: Snowflake,
    ) -> Result<Encryption> {
        if let Some(known) = self.known.get(&guild) {
            return Ok(known);
        }

        let pointer = sqlx::query!(
            "SELECT key_channel_id, key_message_id FROM guild_encryption WHERE guild_id = $1 AND enabled",
            guild as i64
        )
        .fetch_optional(pool)
        .await
        .ctx("read guild encryption pointer")?;

        let Some(row) = pointer else {
            self.known.insert(guild, Encryption::Off);

            return Ok(Encryption::Off);
        };

        let encryption = match fetch(&http, row.key_channel_id, row.key_message_id).await {
            Found::Key(key) => Encryption::On(key),
            Found::Unreachable => Encryption::Lost,
            Found::Gone => Self::retire(pool, http, guild).await?,
        };

        if encryption != Encryption::Lost {
            self.known.insert(guild, encryption);
        }

        Ok(encryption)
    }

    async fn retire(pool: &PgPool, http: impl CacheHttp, guild: Snowflake) -> Result<Encryption> {
        let wiped = store::disable(pool, guild).await?;

        warn!(
            "guild {guild} no longer has its encryption key; encryption disabled and {wiped} unreadable messages erased"
        );

        errorlog::record(
            pool,
            http,
            guild,
            errorlog::Fault::new(
                "The encryption key message was deleted. Encryption has been disabled. Run `encrypt` again to enable encryption again.",
            )
            .detail(format!("{wiped} stored messages erased")),
        )
        .await;

        Ok(Encryption::Off)
    }

    pub async fn protect(
        &self,
        pool: &PgPool,
        http: impl CacheHttp,
        guild: Snowflake,
        plaintext: &str,
    ) -> Result<Option<Vec<u8>>> {
        match self.encryption(pool, http, guild).await? {
            Encryption::Off => Ok(Some(plaintext.as_bytes().to_vec())),
            Encryption::On(key) => Ok(crypto::encrypt(&key, plaintext)),
            Encryption::Lost => Ok(None),
        }
    }
}

pub fn open(key: Option<&Secret>, stored: Option<&[u8]>) -> String {
    let Some(stored) = stored else {
        return String::new();
    };

    let opened = match key {
        Some(key) => crypto::decrypt(key, stored),
        None => String::from_utf8(stored.to_vec()).ok(),
    };

    opened.unwrap_or_else(|| String::from("[unreadable]"))
}

enum Found {
    Key(Secret),
    Gone,
    Unreachable,
}

async fn fetch(http: impl CacheHttp, channel: Option<i64>, message: Option<i64>) -> Found {
    let (Some(channel), Some(message)) = (channel, message) else {
        return Found::Gone;
    };

    let read = ChannelId::new(channel as u64)
        .widen()
        .message(http, MessageId::new(message as u64))
        .await;

    let posted = match read {
        Ok(posted) => posted,
        Err(failure) => {
            return match failure {
                serenity::Error::Http(HttpError::UnsuccessfulRequest(answer))
                    if answer.status_code.as_u16() == 404 =>
                {
                    Found::Gone
                }
                _ => Found::Unreachable,
            };
        }
    };

    let written = std::iter::once(posted.content.as_str()).chain(
        posted
            .embeds
            .iter()
            .filter_map(|embed| embed.description.as_deref()),
    );

    written
        .flat_map(str::lines)
        .find_map(|line| {
            BASE64
                .decode(line.trim().trim_matches('`').trim())
                .ok()?
                .try_into()
                .ok()
        })
        .map_or(Found::Unreachable, Found::Key)
}
