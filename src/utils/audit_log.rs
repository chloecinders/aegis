use chrono::Utc;
use serenity::all::{AuditLogEntry, Context, GuildId, audit_log::Action};

use crate::utils::snowflake_to_timestamp;

pub async fn find_audit_log<F>(
    ctx: &Context,
    guild_id: GuildId,
    log_type: Action,
    f: F,
) -> Option<AuditLogEntry>
where
    F: Fn(&AuditLogEntry) -> bool,
{
    let audit_logs = guild_id
        .audit_logs(&ctx, Some(log_type), None, None, Some(10))
        .await
        .ok()?;

    let found = audit_logs
        .entries
        .iter()
        .filter(move |e| {
            let t = snowflake_to_timestamp(e.id.get());
            let ok = (Utc::now() - t).num_seconds().abs() <= 5;
            ok && f(e)
        })
        .collect::<Vec<_>>();
    found.first().cloned().cloned()
}
