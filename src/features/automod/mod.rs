pub mod cache;
pub mod clause;
pub mod commands;
pub mod controls;
pub mod eval;
pub mod managed;
pub mod readings;
pub mod rule;
pub mod sources;
pub mod store;
pub mod ui;

mod enforce;
mod greet;
mod images;
mod screen;
mod subject;

use std::sync::Arc;

use serenity::async_trait;

use crate::command::registry::Registry;
use crate::platform::discord::dispatch::{Dispatch, MemberCx, MessageCx, Observer};
use crate::platform::discord::interact::Router;
use crate::platform::observe::report::Origin;
use crate::register;

pub struct Automod;

#[async_trait]
impl Observer for Automod {
    fn name(&self) -> &'static str {
        "automod"
    }

    async fn on_message(&self, cx: &MessageCx) {
        if cx.msg.author.bot() || cx.msg.guild_id.is_none() {
            return;
        }

        if let Err(failure) = screen::screen(cx).await {
            cx.app.reporter.record(&failure, origin(cx));
        }
    }

    async fn on_message_edit(&self, cx: &MessageCx) {
        if cx.msg.author.bot() || cx.msg.guild_id.is_none() || cx.msg.edited_timestamp.is_some() {
            return;
        }

        if let Err(failure) = screen::screen_embeds(cx).await {
            cx.app.reporter.record(&failure, origin(cx));
        }
    }

    async fn on_member_add(&self, cx: &MemberCx) {
        if cx.user.bot() {
            return;
        }

        if let Err(failure) = greet::greet(cx).await {
            cx.app.reporter.record(
                &failure,
                Origin {
                    guild: Some(cx.guild.get()),
                    user: Some(cx.user.id.get()),
                    ..Origin::default()
                },
            );
        }
    }
}

fn origin(cx: &MessageCx) -> Origin {
    Origin {
        command: None,
        guild: cx.msg.guild_id.map(|guild| guild.get()),
        channel: Some(cx.msg.channel_id.get()),
        user: Some(cx.msg.author.id.get()),
        message: Some(cx.msg.id.get()),
    }
}

pub fn observe(dispatch: &mut Dispatch) {
    dispatch.add(Arc::new(Automod));
}

pub fn register(registry: &mut Registry) {
    register!(
        registry,
        commands::ocr_check::OcrCheck,
        commands::ocrflush::OcrFlush,
        commands::rule::Rules,
        commands::managed::ManagedRules,
    );
}

pub fn control(router: &mut Router) {
    controls::register(router);
    managed::controls::register(router);
}
