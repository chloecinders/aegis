use std::collections::HashMap;

use serde::Serialize;
use sqlx::{prelude::FromRow, query_as, types::Json};
use tracing::warn;

use crate::{
    SQL,
    utils::{AnyError, LogType},
};

#[derive(Debug, Serialize, Clone, Default)]
pub struct GuildSettings {
    inner: HashMap<u64, Settings>,
    invalid: bool,
}
#[derive(Debug, FromRow, PartialEq, Eq)]
struct GuildSettingsRow {
    guild_id: i64,
    log_bot: Option<bool>,
    log_channel_ids: Option<Json<HashMap<LogType, u64>>>,
}

impl GuildSettings {
    pub fn new() -> Self {
        Self {
            inner: HashMap::new(),
            invalid: true,
        }
    }

    pub fn invalidate(&mut self) {
        self.invalid = true;
    }

    pub async fn get(&mut self, guild: u64) -> Result<Settings, AnyError> {
        if self.invalid {
            let new_data = self.fetch_data().await?;
            self.inner = new_data;
            self.invalid = false;
        }

        match self.inner.get(&guild) {
            Some(s) => Ok(s.clone()),
            None => Err(AnyError::new("guild_not_found")),
        }
    }

    async fn fetch_data(&self) -> Result<HashMap<u64, Settings>, AnyError> {
        match query_as!(
            GuildSettingsRow,
            r#"SELECT
                guild_id,
                log_bot,
                log_channel_ids as "log_channel_ids?: sqlx::types::Json<HashMap<LogType, u64>>"
            FROM guild_settings"#
        )
        .fetch_all(&*SQL)
        .await
        {
            Ok(data) => {
                let mut map: HashMap<u64, Settings> = HashMap::new();

                data.into_iter().for_each(|record| {
                    map.insert(
                        record.guild_id as u64,
                        Settings {
                            log: SettingsLog {
                                log_channel_ids: record
                                    .log_channel_ids
                                    .map(|j| j.0)
                                    .unwrap_or_default(),
                                log_bots: record.log_bot,
                            },
                        },
                    );
                });

                Ok(map)
            }
            Err(err) => {
                warn!("database {err:?}");
                Err(AnyError::new("database_failed"))
            }
        }
    }
}

#[derive(Debug, Serialize, Clone, Default)]
pub struct Settings {
    pub log: SettingsLog,
}

#[derive(Debug, Serialize, Clone, Default)]
pub struct SettingsLog {
    pub log_channel_ids: HashMap<LogType, u64>,
    pub log_bots: Option<bool>,
}
