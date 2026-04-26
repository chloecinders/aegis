use serenity::all::{CacheHttp, EditMember, Guild, GuildId};
use sqlx::query;
use tracing::{error, info, warn};

use crate::SQL;

pub async fn check_expiring_bans(ctx: impl CacheHttp) {
    info!("check_expiring_bans asynchronous task running...");

    let data = match query!(
        r#"
        SELECT id, guild_id, user_id FROM actions WHERE type = 'ban' AND active = true AND expires_at < NOW();
        "#
    ).fetch_all(&*SQL).await {
        Ok(d) => d,
        Err(e) => {
            error!("task check_expiring_bans couldnt fetch necessary data; Err = {e:?}");
            return;
        }
    };

    let mut updated: Vec<String> = vec![];

    for entry in data {
        let Ok(guild) = Guild::get(&ctx, entry.guild_id as u64).await else {
            warn!(
                "task check_expiring_bans couldnt fetch guild; Id = {:?}",
                entry.guild_id
            );
            continue;
        };

        if guild.unban(ctx.http(), entry.user_id as u64).await.is_err() {
            warn!(
                "task check_expiring_bans couldnt unban user; Guild = {:?} Id = {:?}",
                entry.guild_id, entry.user_id
            );
            continue;
        } else {
            updated.push(entry.id);
        }
    }

    if query!(
        r#"
        UPDATE actions SET active = false WHERE id = ANY($1);
        "#,
        &updated
    )
    .execute(&*SQL)
    .await
    .is_err()
    {
        error!(
            "task check_expiring_bans couldnt update entries; entries = {:?}",
            updated
        );
    } else {
        info!("task check_expiring_bans finished");
    }
}

pub async fn check_expiring_timeouts(cache_http: impl CacheHttp) {
    info!("check_expiring_timeouts asynchronous task running...");

    let data = match query!(
        r#"
        SELECT id, guild_id, user_id, expires_at, last_reapplied_at
        FROM actions
        WHERE type = 'timeout'
          AND active = true;
        "#
    )
    .fetch_all(&*SQL)
    .await
    {
        Ok(d) => d,
        Err(e) => {
            error!("task check_expiring_timeouts couldnt fetch necessary data; Err = {e:?}");
            return;
        }
    };

    let mut updated: Vec<String> = vec![];
    let cache_http_ref = &cache_http;
    let now = chrono::Utc::now();

    for entry in data {
        let Ok(mut member) = GuildId::from(entry.guild_id as u64)
            .member(cache_http_ref, entry.user_id as u64)
            .await
        else {
            warn!(
                "task check_expiring_timeouts couldnt fetch member; Guild = {:?} Id = {:?}",
                entry.guild_id, entry.user_id
            );
            continue;
        };

        let remaining = entry
            .expires_at
            .map(|expires_at| expires_at.and_utc() - now);

        let still_active = remaining.map(|d| d.num_seconds() > 0).unwrap_or(true);
        if !still_active {
            continue;
        }

        let needs_reapply = match entry.expires_at {
            None => match entry.last_reapplied_at {
                Some(last) => now.signed_duration_since(last) >= chrono::Duration::days(20),
                None => true,
            },

            Some(expiry) => {
                let remaining = expiry.and_utc() - now;

                if remaining <= chrono::Duration::days(27) {
                    true
                } else {
                    match entry.last_reapplied_at {
                        Some(last) => now.signed_duration_since(last) >= chrono::Duration::days(20),
                        None => true,
                    }
                }
            }
        };

        if needs_reapply {
            let new_timeout = remaining.unwrap_or_else(|| chrono::Duration::days(27));
            let capped_timeout = std::cmp::min(new_timeout, chrono::Duration::days(27));

            let reason = format!(
                "Aegis Managed Mute: log id `{}`. Please use Aegis to unmute to avoid accidental re-application!",
                entry.id
            );
            let edit = EditMember::new()
                .audit_log_reason(reason.as_str())
                .disable_communication_until_datetime((now + capped_timeout).into());

            if let Err(e) = member.edit(cache_http_ref, edit).await {
                warn!(
                    "task check_expiring_timeouts couldnt update timeout; Guild = {:?} Id = {:?} Err = {:?}",
                    entry.guild_id, entry.user_id, e
                );
            } else {
                updated.push(entry.id);
                info!(
                    "reapplied timeout for user {:?} in guild {:?}, now until {:?}",
                    entry.user_id,
                    entry.guild_id,
                    now + capped_timeout
                );
            }
        }
    }

    if !updated.is_empty()
        && let Err(e) = query!(
            r#"UPDATE actions SET last_reapplied_at = NOW() WHERE id = ANY($1);"#,
            &updated
        )
        .execute(&*SQL)
        .await
    {
        error!(
            "task check_expiring_timeouts couldnt update entries; Err = {:?}",
            e
        );
    }

    info!("task check_expiring_timeouts finished");
}
