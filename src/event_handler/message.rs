use serenity::all::{Context, Message};

use crate::{event_handler::Handler, moderation, utils::{command_processing::process, ocr::{ImageData, image_to_string}, rule_cache::Punishment, tinyid}};

pub async fn message(handler: &Handler, ctx: Context, msg: Message) {
    ocr_attachments(&ctx, &msg, handler).await;

    if msg.content.starts_with(handler.prefix.as_str()) && msg.guild_id.is_some() {
        process(handler, ctx.clone(), msg.clone()).await;
        return;
    }

    // let Ok(Some(channel)) = msg.channel(&ctx).await.map(|c| c.guild()) else { return };

    // if let Err(err) = event_engine::run(ctx, channel.guild_id, |lua| {
    //     let event = lua.create_table()?;

    //     event.set("type", "message")?;
    //     event.set("message", message_to_lua_table(&lua, &msg)?)?;

    //     lua.globals().set("event", event)?;

    //     Ok(lua)
    // }).await {
    //     error!("{err:?}");
    // };
}

async fn ocr_attachments(ctx: &Context, msg: &Message, handler: &Handler) {
    if msg.attachments.is_empty() || msg.author.bot {
        return;
    }

    for attachment in &msg.attachments {
        let Ok(req) = reqwest::get(attachment.proxy_url.clone()).await else { continue };
        let Ok(bytes) = req.bytes().await else { continue; };
        let Ok(img) = image::load_from_memory(&bytes) else { continue };

        let img = img.to_rgba8();

        let image_data = ImageData {
            width: img.width().try_into().unwrap_or(0),
            height: img.height().try_into().unwrap_or(0),
            raw: img.into_raw(),
        };

        let image_str = match image_to_string(image_data).await {
            Ok(d) => d,
            Err(_) => {
                continue;
            },
        };

        let rule = {
            let rule_cache = handler.rule_cache.lock().await;
            let rule = rule_cache.matches(msg.guild_id.map(|id| id.get()).unwrap_or(0), image_str);
            rule.cloned()
        };

        if let Some(rule) = rule {
            let current_user_id = ctx.cache.current_user().id;
            let Some(guild_id) = msg.guild_id else { continue };
            let Ok(author) = guild_id.member(ctx, current_user_id).await else { continue };
            let Ok(member) = guild_id.member(ctx, msg.author.id).await else { continue };
            let db_id = tinyid().await;

            match rule.punishment {
                Punishment::Softban { reason, day_clear_amount, silent } => {
                    let _ = moderation::softban(
                        ctx,
                        author,
                        member,
                        guild_id,
                        db_id,
                        format!("Rule {} violation | {}", rule.id, reason),
                        day_clear_amount
                    ).await;
                },
                _ => todo!()
            }

            break;
        }
    }
}
