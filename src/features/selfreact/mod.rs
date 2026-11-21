pub mod cache;
pub mod commands;
pub mod store;

use std::sync::Arc;

use serenity::async_trait;

use crate::command::registry::Registry;
use crate::platform::discord::dispatch::{Dispatch, Observer, ReactionCx};
use crate::platform::observe::report::Origin;
use crate::register;

pub struct SelfReact;

#[async_trait]
impl Observer for SelfReact {
    fn name(&self) -> &'static str {
        "selfreact"
    }

    async fn on_reaction_add(&self, cx: &ReactionCx) {
        let reaction = &cx.reaction;

        let (Some(guild), Some(reactor)) = (reaction.guild_id, reaction.user_id) else {
            return;
        };

        if reaction.message_author_id != Some(reactor)
            || reaction
                .member
                .as_ref()
                .is_some_and(|member| member.user.bot())
        {
            return;
        }

        match cx
            .app
            .self_reactions
            .enabled(&cx.app.pool, guild.get())
            .await
        {
            Ok(true) => {
                let _ = reaction.delete(&cx.ctx.http).await;
            }
            Ok(false) => (),
            Err(failure) => cx.app.reporter.record(
                &failure,
                Origin {
                    command: None,
                    guild: Some(guild.get()),
                    channel: Some(reaction.channel_id.get()),
                    user: Some(reactor.get()),
                    message: Some(reaction.message_id.get()),
                },
            ),
        }
    }
}

pub fn observe(dispatch: &mut Dispatch) {
    dispatch.add(Arc::new(SelfReact));
}

pub fn register(registry: &mut Registry) {
    register!(registry, commands::selfreact::SelfReact);
}
