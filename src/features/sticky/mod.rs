pub mod cache;
pub mod commands;
pub mod store;

use std::sync::Arc;
use std::time::Duration;

use serenity::all::{CreateAllowedMentions, CreateMessage, MessageId};
use serenity::async_trait;

use crate::command::registry::Registry;
use crate::domain::Snowflake;
use crate::platform::cache::Debounce;
use crate::platform::discord::dispatch::{Dispatch, MessageCx, Observer};
use crate::platform::ui::embed::Embed;
use crate::platform::ui::reply;
use crate::platform::ui::tone::Tone;
use crate::register;

pub fn render(sticky: &store::Sticky) -> CreateMessage<'static> {
    if sticky.title.is_none() && sticky.color.is_none() {
        return CreateMessage::new()
            .content(sticky.content.clone())
            .allowed_mentions(CreateAllowedMentions::new().replied_user(false));
    }

    reply::plain(
        &Embed::new(
            sticky
                .title
                .clone()
                .unwrap_or_else(|| String::from("STICKY")),
        )
        .body(sticky.content.clone())
        .tone(Tone::Info)
        .maybe_color(sticky.color),
    )
}

pub struct Sticky {
    cooldown: Debounce<Snowflake>,
}

impl Default for Sticky {
    fn default() -> Self {
        Self {
            cooldown: Debounce::new(2048, Duration::from_secs(8)),
        }
    }
}

#[async_trait]
impl Observer for Sticky {
    fn name(&self) -> &'static str {
        "sticky"
    }

    async fn on_message(&self, cx: &MessageCx) {
        let channel = cx.msg.channel_id;

        let Ok(Some(sticky)) = cx.app.stickies.of(&cx.app.pool, channel.get()).await else {
            return;
        };

        if !self.cooldown.ready(channel.get()) {
            return;
        }

        if let Some(previous) = sticky.last {
            let _ = channel
                .delete_message(&cx.ctx.http, MessageId::new(previous), None)
                .await;
        }

        let Ok(posted) = channel.send_message(&cx.ctx.http, render(&sticky)).await else {
            return;
        };

        let _ = store::mark_posted(&cx.app.pool, channel.get(), posted.id.get()).await;

        cx.app.stickies.forget(channel.get());
    }
}

pub fn observe(dispatch: &mut Dispatch) {
    dispatch.add(Arc::new(Sticky::default()));
}

pub fn register(registry: &mut Registry) {
    register!(registry, commands::sticky::SetSticky);
}
