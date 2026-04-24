use serenity::{
    all::{Context, Message},
    futures::{StreamExt, stream::FuturesUnordered},
};

use crate::{
    event_handler::Handler,
    moderation,
    utils::{
        command_processing::process,
        ocr::{ImageData, image_to_string_with_rotation, likely_has_text},
        rule_cache::Punishment,
        tinyid,
    },
};

pub async fn message(handler: &Handler, ctx: Context, msg: Message) {
    ocr_attachments(&ctx, &msg, handler).await;

    if msg.content.starts_with(handler.prefix.as_str()) && msg.guild_id.is_some() {
        process(handler, ctx.clone(), msg.clone()).await;
        return;
    }
}

async fn ocr_attachments(ctx: &Context, msg: &Message, handler: &Handler) {
    if msg.attachments.is_empty() || msg.author.bot {
        return;
    }

    let Some(guild_id) = msg.guild_id else {
        return;
    };

    if let Ok(member) = guild_id.member(ctx, msg.author.id).await {
        if let Ok(perms) = member.permissions(ctx) {
            if perms.contains(serenity::all::Permissions::MANAGE_MESSAGES)
                || perms.contains(serenity::all::Permissions::ADMINISTRATOR)
            {
                return;
            }
        }
    }

    let mut handles = vec![];
    let guild_id_u64 = guild_id.get();

    for attachment in msg.attachments.clone().into_iter() {
        let rule_cache = handler.rule_cache.clone();

        handles.push(tokio::spawn(async move {
            let Ok(req) = reqwest::get(attachment.proxy_url.clone()).await else {
                return None;
            };
            let Ok(bytes) = req.bytes().await else {
                return None;
            };
            let Ok(img) = image::load_from_memory(&bytes) else {
                return None;
            };

            let img = img.to_rgba8();

            let image_data = ImageData {
                width: img.width().try_into().unwrap_or(0),
                height: img.height().try_into().unwrap_or(0),
                raw: img.into_raw(),
            };

            if !likely_has_text(&image_data).await.is_ok_and(|b| b) {
                return None;
            }

            let image_str = match image_to_string_with_rotation(&image_data).await {
                Ok(d) => d,
                Err(_) => {
                    return None;
                }
            };

            let rule = {
                let rule_cache = rule_cache.lock().await;
                let rule = rule_cache.matches(guild_id_u64, image_str);
                rule.cloned()
            };

            return rule;
        }));
    }

    let mut futures: FuturesUnordered<_> = handles.into_iter().collect();

    while let Some(Ok(Some(rule))) = futures.next().await {
        let current_user_id = ctx.cache.current_user().id;
        let Some(guild_id) = msg.guild_id else {
            return;
        };
        let Ok(author) = guild_id.member(ctx, current_user_id).await else {
            return;
        };
        let Ok(member) = guild_id.member(ctx, msg.author.id).await else {
            return;
        };
        let db_id = tinyid().await;

        let _ = msg.delete(ctx).await;
        let formatted_reason = format!(
            "Rule {} violation | {}",
            rule.id,
            match &rule.punishment {
                Punishment::Warn { reason, .. }
                | Punishment::Softban { reason, .. }
                | Punishment::Kick { reason, .. }
                | Punishment::Ban { reason, .. }
                | Punishment::Mute { reason, .. }
                | Punishment::Log { reason, .. } => reason,
            }
        );

        let guild_name = {
            match guild_id.to_partial_guild(&ctx).await {
                Ok(p) => p.name.clone(),
                Err(_) => String::from("UNKNOWN_GUILD"),
            }
        };

        let time_string = |duration_seconds: u64| -> String {
            if duration_seconds == 0 {
                return String::from("permanent");
            }
            let duration =
                chrono::TimeDelta::try_seconds(duration_seconds as i64).unwrap_or_default();
            let (time, mut unit) = match () {
                _ if (duration.num_days() as f64 / 365.0).fract() == 0.0
                    && duration.num_days() >= 365 =>
                {
                    (duration.num_days() / 365, String::from("year"))
                }
                _ if (duration.num_days() as f64 / 30.0).fract() == 0.0
                    && duration.num_days() >= 30 =>
                {
                    (duration.num_days() / 30, String::from("month"))
                }
                _ if duration.num_days() != 0 => (duration.num_days(), String::from("day")),
                _ if duration.num_hours() != 0 => (duration.num_hours(), String::from("hour")),
                _ if duration.num_minutes() != 0 => {
                    (duration.num_minutes(), String::from("minute"))
                }
                _ if duration.num_seconds() != 0 => {
                    (duration.num_seconds(), String::from("second"))
                }
                _ => (0, String::new()),
            };
            if time > 1 {
                unit.push('s');
            }
            format!("for {time} {unit}")
        };

        macro_rules! send_dm {
            ($silent:expr, $title:expr) => {
                send_dm!($silent, $title, String::new())
            };
            ($silent:expr, $title:expr, $duration:expr) => {
                if !$silent {
                    use serenity::all::{CreateEmbed, CreateMessage};
                    let duration_text = if $duration.is_empty() {
                        String::new()
                    } else {
                        format!(" | Duration: {}", $duration)
                    };
                    let desc = format!(
                        "**{}**\n-# Server: {}{}\n```\n{}\n```",
                        $title, guild_name, duration_text, formatted_reason
                    );
                    let dm = CreateMessage::new().add_embed(
                        CreateEmbed::new()
                            .description(desc)
                            .color(crate::constants::BRAND_BLUE),
                    );
                    let _ = msg.author.direct_message(&ctx, dm).await;
                }
            };
        }

        match rule.punishment {
            Punishment::Warn { reason: _, silent } => {
                send_dm!(silent, "WARNED");
                let _ = moderation::warn_member(
                    &ctx,
                    author,
                    member,
                    guild_id,
                    db_id,
                    formatted_reason,
                    (None, None),
                )
                .await;
            }
            Punishment::Softban {
                reason: _,
                day_clear_amount,
                silent,
            } => {
                send_dm!(silent, "SOFTBANNED");
                let _ = moderation::softban(
                    ctx,
                    author,
                    member,
                    guild_id,
                    db_id,
                    formatted_reason,
                    day_clear_amount,
                    (None, None),
                )
                .await;
            }
            Punishment::Kick { reason: _, silent } => {
                send_dm!(silent, "KICKED");
                let _ = moderation::kick_member(
                    &ctx,
                    author,
                    member,
                    guild_id,
                    db_id,
                    formatted_reason,
                    (None, None),
                )
                .await;
            }
            Punishment::Ban {
                reason: _,
                day_clear_amount,
                duration,
                silent,
            } => {
                send_dm!(silent, "BANNED", time_string(duration));
                let _ = moderation::ban_member(
                    ctx,
                    author,
                    member,
                    guild_id,
                    db_id,
                    formatted_reason,
                    day_clear_amount,
                    chrono::TimeDelta::try_seconds(duration as i64).unwrap_or_default(),
                    (None, None),
                )
                .await;
            }
            Punishment::Mute {
                reason: _,
                duration,
                silent,
            } => {
                send_dm!(silent, "MUTED", time_string(duration));
                let _ = moderation::mute_member(
                    ctx,
                    author,
                    member,
                    guild_id,
                    db_id,
                    formatted_reason,
                    chrono::TimeDelta::try_seconds(duration as i64).unwrap_or_default(),
                    (None, None),
                )
                .await;
            }
            Punishment::Log {
                reason: _,
                channel_id,
            } => {
                use serenity::all::{
                    ChannelId, CreateAllowedMentions, CreateEmbed, CreateMessage, Mentionable,
                };
                let reply = CreateMessage::new()
                    .add_embed(
                        CreateEmbed::new()
                            .description(format!(
                                "**OCR RULE TRIGGERED**\n-# Log ID: `{}` | Actor: {} | Target: {} | Rule: {}\n```\n{}\n```",
                                db_id,
                                author.mention(),
                                member.mention(),
                                rule.name.to_uppercase(),
                                formatted_reason
                            ))
                            .color(crate::constants::BRAND_BLUE)
                    )
                    .allowed_mentions(CreateAllowedMentions::new().replied_user(false));

                if let Ok(m) = ChannelId::new(channel_id).send_message(ctx, reply).await {
                    let _ = sqlx::query!(
                        "INSERT INTO log_messages_context (message_id, guild_id, target_id, moderator_id, db_id, content) VALUES ($1, $2, $3, $4, $5, $6)",
                        m.id.get() as i64,
                        guild_id.get() as i64,
                        member.user.id.get() as i64,
                        author.user.id.get() as i64,
                        Some(db_id),
                        None::<Vec<u8>>
                    )
                    .execute(&*crate::SQL)
                    .await;
                }
            }
        }

        break;
    }
}
