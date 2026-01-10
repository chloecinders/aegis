use mlua::Lua;
use serenity::{all::{CacheHttp, Context, CreateEmbed, CreateMessage, Embed, GuildChannel, GuildId, Message}, model::guild};
use thiserror::Error;
use tokio::{runtime::{Handle, Runtime}, task::LocalSet};
use tracing::error;

use crate::{constants::BRAND_BLUE, utils::consume_serenity_error};

#[derive(Error, Debug)]
pub enum CreateError {
    #[error("{0:?}")]
    MLua(#[from] mlua::Error),
    #[error("Message channel not found or is not guild channel")]
    ChannelNotFound,
}

async fn create_base_engine(ctx: Context, guild_id: GuildId) -> Result<Lua, CreateError> {
    let lua = Lua::new();

    lua.set_hook(mlua::HookTriggers { every_nth_instruction: Some(10_000), ..Default::default() }, |_lua, _debug| {
        Err(mlua::Error::RuntimeError("instruction limit hit, please upload shorter scripts".into()))
    })?;

    let log_ctx_clone = ctx.clone();
    let log = lua.create_function(move |_, (channel_id, message): (String, String)| {
        let log_ctx_clone = log_ctx_clone.clone();
        let guild_id = guild_id;

        tokio::spawn(async move {
            let channel_id = match channel_id.parse::<u64>() {
                Ok(id) => id,
                Err(_) => return,
            };

            let Some(channel) = log_ctx_clone
                .http()
                .get_channel(channel_id.into())
                .await
                .ok()
                .and_then(|c| c.guild())
            else {
                return;
            };

            if channel.guild_id != guild_id {
                return;
            }

            let _ = channel.send_message(&log_ctx_clone, CreateMessage::new().add_embed(
                CreateEmbed::new()
                    .color(BRAND_BLUE)
                    .description(format!("**EVENT ENGINE LOG**\n{message}\n")),
            )).await;
        });

        Ok(())
    })?;

    lua.globals().set("log", log)?;

    Ok(lua)
}

pub async fn run(ctx: Context, guild_id: GuildId, construct_lua: impl FnOnce(Lua) -> Result<Lua, CreateError>) -> Result<(), CreateError> {
    let engine = create_base_engine(ctx, guild_id).await?;
    let engine = construct_lua(engine)?;

    engine.load(r#"
    if event.type == "message" and event.message.content == "fuck you" then
        log("1440114797956960451", "fuck you too")

        set_flag(event.message.author.id, "sent_images", 1)
    end
    "#).exec()?;

    Ok(())
}

pub fn message_to_lua_table(lua: &Lua, msg: &Message) -> mlua::Result<mlua::Table> {
    let author_table = lua.create_table()?;
    author_table.set("id", msg.author.id.get().to_string())?;
    author_table.set("name", msg.author.name.clone())?;
    author_table.set("bot", msg.author.bot)?;

    let msg_table = lua.create_table()?;
    msg_table.set("id", msg.id.get().to_string())?;
    msg_table.set("channel_id", msg.channel_id.get().to_string())?;
    msg_table.set("content", msg.content.clone())?;
    msg_table.set("timestamp", msg.timestamp.to_rfc3339())?;
    msg_table.set("edited_timestamp", msg.edited_timestamp.map(|t| t.to_rfc3339()))?;
    msg_table.set("author", author_table)?;

    let attachments_table = lua.create_table()?;
    for (i, att) in msg.attachments.iter().enumerate() {
        attachments_table.set(i + 1, att.url.clone())?;
    }
    msg_table.set("attachments", attachments_table)?;

    Ok(msg_table)
}
