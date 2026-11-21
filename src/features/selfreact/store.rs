use sqlx::PgPool;

use crate::command::error::{Ctx, Result};
use crate::domain::Snowflake;
use crate::features::settings::store;

pub async fn enabled(pool: &PgPool, guild: Snowflake) -> Result<bool> {
    let row = sqlx::query!(
        "SELECT block_self_reactions FROM guild_settings WHERE guild_id = $1",
        guild as i64
    )
    .fetch_optional(pool)
    .await
    .ctx("read self reaction setting")?;

    Ok(row.is_some_and(|row| row.block_self_reactions))
}

pub async fn set(pool: &PgPool, guild: Snowflake, enabled: bool) -> Result<()> {
    store::enroll(pool, guild).await?;

    sqlx::query!(
        "UPDATE guild_settings SET block_self_reactions = $2 WHERE guild_id = $1",
        guild as i64,
        enabled
    )
    .execute(pool)
    .await
    .ctx("save self reaction setting")?;

    Ok(())
}
