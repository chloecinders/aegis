use std::time::Duration;

use sqlx::PgPool;

use crate::command::error::Result;
use crate::domain::Snowflake;
use crate::features::selfreact::store;
use crate::platform::cache::Cache;

pub struct SelfReactions {
    enabled: Cache<Snowflake, bool>,
}

impl Default for SelfReactions {
    fn default() -> Self {
        Self::new()
    }
}

impl SelfReactions {
    pub fn new() -> Self {
        Self {
            enabled: Cache::new(4096, Some(Duration::from_secs(900))),
        }
    }

    pub fn forget(&self, guild: Snowflake) {
        self.enabled.remove(&guild);
    }

    pub async fn enabled(&self, pool: &PgPool, guild: Snowflake) -> Result<bool> {
        if let Some(enabled) = self.enabled.get(&guild) {
            return Ok(enabled);
        }

        let loaded = store::enabled(pool, guild).await?;

        self.enabled.insert(guild, loaded);

        Ok(loaded)
    }
}
