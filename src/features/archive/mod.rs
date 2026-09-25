pub mod automod;
pub mod cache;
pub mod commands;
pub mod controls;
pub mod deletion;
pub mod edit;
pub mod secrets;
pub mod store;

#[cfg(feature = "web")]
pub mod transcript;

use std::sync::Arc;

use serenity::all::MessageType;
use serenity::async_trait;

use crate::command;
use crate::command::registry::Registry;
use crate::platform::discord::dispatch::{
    BulkDeletionCx, DeletionCx, Dispatch, MessageCx, Observer,
};
use crate::platform::discord::interact::Router;
use crate::platform::discord::partial::PartialMessage;
use crate::platform::ui::embed;

pub struct Storable {
    pub message: cache::Cached,
    pub body: Option<Vec<u8>>,
    pub system: bool,
}

pub fn reply_line(message: &PartialMessage) -> Option<String> {
    message.referenced_message_id.map(|parent| {
        format!(
            "Reply: `{parent}` [jump]({})",
            embed::jump(
                message.guild_id.unwrap_or_default(),
                message.channel_id,
                parent
            )
        )
    })
}

async fn note_removal(
    cx: &DeletionCx,
    guild: serenity::all::GuildId,
) -> command::error::Result<()> {
    store::removed(
        &cx.app.pool,
        guild.get(),
        cx.message.get(),
        store::Removal::Manual,
        None,
    )
    .await
}

pub struct Archive;

#[async_trait]
impl Observer for Archive {
    fn name(&self) -> &'static str {
        "archive"
    }

    async fn on_message_edit(&self, cx: &MessageCx) {
        let Some(guild) = cx.msg.guild_id else {
            return;
        };

        if let Err(failure) = edit::record(&cx.app, &cx.ctx, guild, &cx.msg).await {
            cx.app
                .reporter
                .note("could not archive a message edit", failure.to_string());
        }
    }

    async fn on_message_delete(&self, cx: &DeletionCx) {
        let Some(guild) = cx.guild else {
            return;
        };

        if let Err(failure) = note_removal(cx, guild).await {
            cx.app
                .reporter
                .note("could not log a removed message", failure.to_string());
        }

        let recorded = deletion::record(&cx.app, &cx.ctx, guild, cx.channel, cx.message).await;

        if let Err(failure) = recorded {
            cx.app
                .reporter
                .note("could not archive a deleted message", failure.to_string());
        }
    }

    async fn on_message_delete_bulk(&self, cx: &BulkDeletionCx) {
        let Some(guild) = cx.guild else {
            return;
        };

        if let Err(failure) = deletion::bulk(cx, guild).await {
            cx.app
                .reporter
                .note("could not archive a bulk deletion", failure.to_string());
        }
    }

    async fn on_message(&self, cx: &MessageCx) {
        let Some(guild) = cx.msg.guild_id else {
            return;
        };

        if cx.msg.kind == MessageType::AutoModAction {
            automod::record(cx, guild.get()).await;

            return;
        }

        let system = cx.msg.kind == MessageType::MemberJoin;
        let mut message = PartialMessage::from(cx.msg.as_ref());

        if system {
            message.content = format!("Joined: {}", message.author.name);
        }

        let message = Arc::new(message);

        cx.app.recent.remember(Arc::clone(&message));

        let kept = cx
            .app
            .secrets
            .protect(&cx.app.pool, &cx.ctx, guild.get(), &message.content)
            .await;

        match kept {
            Ok(body) => cx.app.messages.send(Storable {
                message,
                body,
                system,
            }),
            Err(failure) => cx
                .app
                .reporter
                .note("could not archive a message", failure.to_string()),
        }
    }
}

pub fn observe(dispatch: &mut Dispatch) {
    dispatch.add(Arc::new(Archive));
}

pub fn register(registry: &mut Registry) {
    crate::register!(
        registry,
        commands::encrypt::Encrypt,
        commands::msgdbg::MsgDbg,
        commands::purge::Purge,
    );

    crate::register_slash!(registry, commands::snipe::Snipe);

    #[cfg(feature = "web")]
    crate::register!(registry, commands::message_log::MessageLog);
}

pub fn control(router: &mut Router) {
    controls::register(router);
}
