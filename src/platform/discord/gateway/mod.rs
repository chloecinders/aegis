mod audit;

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use serenity::all::{
    ActivityData, AuditLogEntry, ChannelId, Client, ClientBuilder, Context, EventHandler,
    GatewayIntents, GuildId, HttpBuilder, Interaction, Member, Message, MessageId,
    MessageUpdateEvent, OnlineStatus, Ready, Settings, User,
};
use serenity::async_trait;
use tracing::info;

use crate::app::App;
use crate::command::{amend, pipeline, retract};
use crate::features::punishments::sync;
use crate::platform::discord::dispatch::{
    BulkDeletionCx, DeletionCx, Dispatch, MemberCx, MessageCx, VoiceCx,
};
use crate::platform::discord::interact;

pub struct Gateway {
    app: Arc<App>,
    dispatch: Arc<Dispatch>,
    booted: AtomicBool,
}

fn revised(event: &MessageUpdateEvent) -> Option<Message> {
    if event.author.is_none() || event.content.is_none() {
        return None;
    }

    let mut message = Message::default();

    event.apply_to_message(&mut message);

    Some(message)
}

#[async_trait]
impl EventHandler for Gateway {
    async fn ready(&self, ctx: Context, ready: Ready) {
        ctx.set_presence(
            Some(ActivityData::watching(format!(
                "Moderating Members... | {}help",
                self.app.prefix()
            ))),
            OnlineStatus::Online,
        );

        info!(
            "connected as {} across {} shards",
            ready.user.name,
            ready.shard.map(|shard| shard.total).unwrap_or(1)
        );

        if self.booted.swap(true, Ordering::SeqCst) {
            return;
        }

        tokio::spawn(sync::on_boot(Arc::clone(&self.app), Arc::clone(&ctx.http)));

        #[cfg(feature = "web")]
        if self.app.config.discord_client_id.is_some() {
            let http = Arc::clone(&ctx.http);

            tokio::spawn(async move { crate::web::entrypoint::install(&http).await });
        }
    }

    async fn message(&self, ctx: Context, message: Message) {
        if message.author.bot {
            return;
        }

        let message = Arc::new(message);
        let cx = MessageCx {
            app: Arc::clone(&self.app),
            ctx: ctx.clone(),
            msg: Arc::clone(&message),
        };

        self.dispatch.message(&cx).await;
        pipeline::guarded(Arc::clone(&self.app), ctx, message).await;
    }

    async fn message_delete(
        &self,
        ctx: Context,
        channel: ChannelId,
        message: MessageId,
        guild: Option<GuildId>,
    ) {
        let cx = DeletionCx {
            app: Arc::clone(&self.app),
            ctx: ctx.clone(),
            guild,
            channel,
            message,
        };

        self.dispatch.message_delete(&cx).await;
        retract::withdraw(Arc::clone(&self.app), ctx, channel, message).await;
    }

    async fn message_delete_bulk(
        &self,
        ctx: Context,
        channel: ChannelId,
        messages: Vec<MessageId>,
        guild: Option<GuildId>,
    ) {
        let cx = BulkDeletionCx {
            app: Arc::clone(&self.app),
            ctx,
            guild,
            channel,
            messages,
        };

        self.dispatch.message_delete_bulk(&cx).await;
    }

    async fn guild_member_addition(&self, ctx: Context, member: Member) {
        let cx = MemberCx {
            app: Arc::clone(&self.app),
            ctx,
            guild: member.guild_id,
            user: member.user.clone(),
            member: Some(member),
            previous: None,
        };

        self.dispatch.member_add(&cx).await;
    }

    async fn guild_member_removal(
        &self,
        ctx: Context,
        guild: GuildId,
        user: User,
        member: Option<Member>,
    ) {
        let cx = MemberCx {
            app: Arc::clone(&self.app),
            ctx,
            guild,
            user,
            member,
            previous: None,
        };

        self.dispatch.member_remove(&cx).await;
    }

    async fn guild_member_update(
        &self,
        ctx: Context,
        previous: Option<Member>,
        member: Option<Member>,
        event: serenity::all::GuildMemberUpdateEvent,
    ) {
        let cx = MemberCx {
            app: Arc::clone(&self.app),
            ctx,
            guild: event.guild_id,
            user: event.user.clone(),
            member,
            previous,
        };

        self.dispatch.member_update(&cx).await;
    }

    async fn voice_state_update(
        &self,
        ctx: Context,
        previous: Option<serenity::all::VoiceState>,
        current: serenity::all::VoiceState,
    ) {
        let Some(guild) = current.guild_id else {
            return;
        };

        let cx = VoiceCx {
            app: Arc::clone(&self.app),
            ctx,
            guild,
            user: current.user_id,
            bot: current
                .member
                .as_ref()
                .is_some_and(|member| member.user.bot),
            previous,
            current,
        };

        self.dispatch.voice_state(&cx).await;
    }

    async fn interaction_create(&self, ctx: Context, interaction: Interaction) {
        match interaction {
            Interaction::Component(component) => {
                interact::dispatch(Arc::clone(&self.app), ctx, component).await
            }
            Interaction::Modal(modal) => {
                interact::submitted(Arc::clone(&self.app), ctx, modal).await
            }
            #[cfg(feature = "web")]
            Interaction::Command(command) => crate::web::entrypoint::launched(&ctx, &command).await,
            _ => (),
        }
    }

    async fn guild_audit_log_entry_create(
        &self,
        ctx: Context,
        entry: AuditLogEntry,
        guild: GuildId,
    ) {
        audit::record(&self.app, &ctx, entry, guild).await;
    }

    async fn message_update(
        &self,
        ctx: Context,
        _old: Option<Message>,
        new: Option<Message>,
        event: MessageUpdateEvent,
    ) {
        let Some(message) = new.or_else(|| revised(&event)) else {
            return;
        };

        if message.author.bot {
            return;
        }

        let message = Arc::new(message);
        let cx = MessageCx {
            app: Arc::clone(&self.app),
            ctx: ctx.clone(),
            msg: Arc::clone(&message),
        };

        self.dispatch.message_edit(&cx).await;
        amend::reconsider(Arc::clone(&self.app), ctx, message).await;
    }
}

pub async fn build(
    app: Arc<App>,
    dispatch: Dispatch,
    token: &str,
) -> Result<Client, Box<serenity::Error>> {
    let mut cache = Settings::default();

    cache.max_messages = 0;

    let http = HttpBuilder::new(token)
        .client(crate::platform::http::discord())
        .build();

    ClientBuilder::new_with_http(
        http,
        GatewayIntents::non_privileged()
            .union(GatewayIntents::GUILD_MEMBERS)
            .union(GatewayIntents::MESSAGE_CONTENT),
    )
    .event_handler(Gateway {
        app,
        dispatch: Arc::new(dispatch),
        booted: AtomicBool::new(false),
    })
    .cache_settings(cache)
    .await
    .map_err(Box::new)
}
