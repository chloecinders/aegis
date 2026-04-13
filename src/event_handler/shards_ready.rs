use std::{fs, sync::Arc};

use serenity::all::{ActivityData, Context, Permissions, RoleId};
use sqlx::query;
use tracing::{error, info};

use crate::{
    BOT_CONFIG, GUILD_SETTINGS, SQL, event_handler::Handler,
    utils::cache::permission_cache::CommandPermissionRequest,
};

pub async fn shards_ready(handler: &Handler, ctx: Context, _total_shards: u32) {
    finish_update(&ctx).await;
    check_whitelist(&ctx).await;
    update_guild_settings(&ctx).await;
    fill_message_cache(handler, &ctx).await;

    let handler_clone = handler.clone();
    let ctx_clone = ctx.clone();
    tokio::spawn(async move {
        fill_permission_cache(&handler_clone, &ctx_clone).await;
    });

    set_activity(handler, &ctx).await;
}

pub async fn finish_update(ctx: &Context) {
    let ids = {
        if let Some(arg) = std::env::args()
            .collect::<Vec<String>>()
            .iter()
            .find(|a| a.starts_with("--id"))
        {
            let Some(ids) = arg.split("=").last() else {
                return;
            };

            ids.to_string()
        } else if let Ok(ids) = fs::read_to_string("./update.txt") {
            let _ = fs::remove_file("./update.txt");
            ids
        } else {
            return;
        }
    };

    let mut parts = ids.split(':');

    let (channel_id, msg_id, hash) = match (parts.next(), parts.next(), parts.next()) {
        (Some(a), Some(b), Some(c)) => (a, b, Some(c)),
        (Some(a), Some(b), None) => (a, b, None),
        _ => {
            return;
        }
    };

    let Ok(channel) = ctx
        .http
        .get_channel(channel_id.parse::<u64>().unwrap().into())
        .await
    else {
        return;
    };

    let Ok(message) = channel
        .id()
        .message(ctx, msg_id.parse::<u64>().unwrap())
        .await
    else {
        return;
    };

    info!("Replying to update command; channel = {channel:?} message = {message:?}");

    let msg = match hash {
        Some(h) => format!("Updated to `{}`", &h[0..7.min(h.len())]),
        None => String::from("Update finished!"),
    };

    let _ = message.reply(ctx, msg).await;
}

pub async fn update_guild_settings(ctx: &Context) {
    info!("Adding missing guilds to guild_settings");
    let guild_ids: Vec<String> = ctx
        .cache
        .guilds()
        .iter()
        .map(|g| format!("({})", g.get()))
        .collect();

    let query = format!(
        r#"INSERT INTO guild_settings (guild_id) VALUES {} ON CONFLICT (guild_id) DO NOTHING;"#,
        guild_ids.join(", ")
    );

    if let Err(err) = sqlx::query(&query).execute(&*SQL).await {
        error!("Couldnt add missing guilds to guild_settings; err = {err:?}")
    }

    {
        let mut settings = GUILD_SETTINGS.lock().await;
        settings.invalidate();
    }
}

pub async fn check_whitelist(ctx: &Context) {
    if BOT_CONFIG.whitelist_enabled.is_none_or(|b| !b) {
        return;
    }

    for guild in ctx.cache.guilds() {
        if BOT_CONFIG
            .whitelist
            .as_ref()
            .is_none_or(|ids| !ids.contains(&guild.get()))
            && let Err(err) = ctx.http.leave_guild(guild).await
        {
            error!(
                "Could not leave non-whitelisted guild! err = {err:?}; id = {}",
                guild.get()
            );
        }
    }
}

pub async fn fill_message_cache(handler: &Handler, ctx: &Context) {
    let existing_data = match query!("SELECT * FROM message_cache_store")
        .fetch_all(&*SQL)
        .await
    {
        Ok(r) => r,
        Err(err) => {
            error!("Couldnt fetch latest message cache counts; err = {err:?}");
            return;
        }
    };

    let mut lock = handler.message_cache.lock().await;

    for guild in ctx.cache.guilds() {
        let Some(cached) = guild.to_guild_cached(&ctx.cache) else {
            continue;
        };

        for id in cached.channels.keys() {
            lock.assign_count(id.get(), 100);
        }
    }

    for record in existing_data {
        lock.assign_count(record.channel_id as u64, record.message_count as usize);
    }
}

pub async fn fill_permission_cache(handler: &Handler, ctx: &Context) {
    let commands = handler.commands.clone();
    let Some(ban_command) = commands.iter().find(|c| c.get_name() == "ban").cloned() else {
        return;
    };

    let bot_id = ctx.cache.current_user().id;

    for guild_id in ctx.cache.guilds() {
        let (owner_id, roles, staff_members, channel_id, overwrites, current_user) = {
            let Some(guild) = ctx.cache.guild(guild_id) else {
                continue;
            };

            let owner_id = guild.owner_id;
            let roles = Arc::new(guild.roles.clone());

            let channel_data = {
                if let Some(channel_id) = guild
                    .system_channel_id
                    .or(guild.widget_channel_id)
                    .or(guild.rules_channel_id)
                    .or(guild.public_updates_channel_id)
                {
                    guild
                        .channels
                        .get(&channel_id)
                        .map(|c| (c.id, Arc::new(c.permission_overwrites.clone())))
                } else {
                    None
                }
            };

            let Some((channel_id, overwrites)) = channel_data else {
                continue;
            };

            let mut valid_roles: Vec<RoleId> = Vec::new();
            for (id, role) in &*roles {
                if role.permissions.contains(Permissions::MANAGE_MESSAGES)
                    || role.permissions.contains(Permissions::BAN_MEMBERS)
                    || role.permissions.contains(Permissions::KICK_MEMBERS)
                    || role.permissions.contains(Permissions::ADMINISTRATOR)
                {
                    valid_roles.push(id.clone());
                }
            }

            let Some(current_user) = guild.members.get(&bot_id).cloned() else {
                continue;
            };

            let staff = guild
                .members
                .values()
                .filter(|m| m.roles.iter().any(|r| valid_roles.contains(&r)))
                .cloned()
                .collect::<Vec<_>>();

            (owner_id, roles, staff, channel_id, overwrites, current_user)
        };

        for member in staff_members {
            let mut cache = handler.permission_cache.lock().await;

            cache
                .can_run(
                    CommandPermissionRequest {
                        current_user: current_user.clone(),
                        command: ban_command.clone(),
                        member,
                        guild_id,
                        owner_id,
                        roles: Arc::clone(&roles),
                        channel_id,
                        overwrites: Arc::clone(&overwrites),
                        handler: handler.clone(),
                    },
                    None,
                )
                .await;
        }
    }
}

async fn set_activity(handler: &Handler, ctx: &Context) {
    ctx.set_activity(Some(ActivityData::watching(format!(
        "Moderating Members... | {}help",
        handler.prefix
    ))));
}
