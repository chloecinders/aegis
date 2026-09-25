mod audit;

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use serenity::all::{
    ActivityData, AuditLogEntry, Client, ClientBuilder, Context, EventHandler, FullEvent,
    GatewayIntents, GenericChannelId, GuildId, GuildMemberUpdateEvent, HttpBuilder, Interaction,
    Member, Message, MessageId, OnlineStatus, Ready, Settings, Token, User, VoiceState,
};
use serenity::async_trait;
use tracing::info;

use crate::app::App;
use crate::command::{amend, pipeline, retract, slash};
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

#[async_trait]
impl EventHandler for Gateway {
    async fn dispatch(&self, ctx: &Context, event: &FullEvent) {
        match event {
            FullEvent::Ready { data_about_bot, .. } => self.ready(ctx, data_about_bot),
            FullEvent::Message { new_message, .. } => self.message(ctx, new_message.clone()).await,
            FullEvent::MessageDelete {
                channel_id,
                deleted_message_id,
                guild_id,
                ..
            } => {
                self.message_delete(ctx, *channel_id, *deleted_message_id, *guild_id)
                    .await
            }
            FullEvent::MessageDeleteBulk {
                channel_id,
                multiple_deleted_messages_ids,
                guild_id,
                ..
            } => {
                self.message_delete_bulk(
                    ctx,
                    *channel_id,
                    multiple_deleted_messages_ids.clone(),
                    *guild_id,
                )
                .await
            }
            FullEvent::GuildMemberAddition { new_member, .. } => {
                self.guild_member_addition(ctx, new_member.clone()).await
            }
            FullEvent::GuildMemberRemoval {
                guild_id,
                user,
                member_data_if_available,
                ..
            } => {
                self.guild_member_removal(
                    ctx,
                    *guild_id,
                    user.clone(),
                    member_data_if_available.clone(),
                )
                .await
            }
            FullEvent::GuildMemberUpdate {
                old_if_available,
                new,
                event,
                ..
            } => {
                self.guild_member_update(ctx, old_if_available.clone(), new.clone(), event)
                    .await
            }
            FullEvent::VoiceStateUpdate { old, new, .. } => {
                self.voice_state_update(ctx, old.clone(), new.clone()).await
            }
            FullEvent::InteractionCreate { interaction, .. } => {
                self.interaction_create(ctx, interaction.clone()).await
            }
            FullEvent::GuildAuditLogEntryCreate {
                entry, guild_id, ..
            } => {
                self.guild_audit_log_entry_create(ctx, entry.clone(), *guild_id)
                    .await
            }
            FullEvent::MessageUpdate { event, .. } => {
                self.message_update(ctx, event.message.clone()).await
            }
            _ => (),
        }
    }
}

impl Gateway {
    fn ready(&self, ctx: &Context, ready: &Ready) {
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
            ready.shard.map_or(1, |shard| shard.total.get())
        );

        if self.booted.swap(true, Ordering::SeqCst) {
            return;
        }

        tokio::spawn(sync::on_boot(Arc::clone(&self.app), Arc::clone(&ctx.http)));
        tokio::spawn(slash::run::install(
            Arc::clone(&self.app),
            Arc::clone(&ctx.http),
        ));
    }

    async fn message(&self, ctx: &Context, message: Message) {
        if message.author.bot() {
            return;
        }

        let message = Arc::new(message);
        let cx = MessageCx {
            app: Arc::clone(&self.app),
            ctx: ctx.clone(),
            msg: Arc::clone(&message),
        };

        self.dispatch.message(&cx).await;
        pipeline::guarded(Arc::clone(&self.app), ctx.clone(), message).await;
    }

    async fn message_delete(
        &self,
        ctx: &Context,
        channel: GenericChannelId,
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
        retract::withdraw(Arc::clone(&self.app), ctx.clone(), channel, message).await;
    }

    async fn message_delete_bulk(
        &self,
        ctx: &Context,
        channel: GenericChannelId,
        messages: Vec<MessageId>,
        guild: Option<GuildId>,
    ) {
        let cx = BulkDeletionCx {
            app: Arc::clone(&self.app),
            ctx: ctx.clone(),
            guild,
            channel,
            messages,
        };

        self.dispatch.message_delete_bulk(&cx).await;
    }

    async fn guild_member_addition(&self, ctx: &Context, member: Member) {
        let cx = MemberCx {
            app: Arc::clone(&self.app),
            ctx: ctx.clone(),
            guild: member.guild_id,
            user: member.user.clone(),
            member: Some(member),
            previous: None,
        };

        self.dispatch.member_add(&cx).await;
    }

    async fn guild_member_removal(
        &self,
        ctx: &Context,
        guild: GuildId,
        user: User,
        member: Option<Member>,
    ) {
        let cx = MemberCx {
            app: Arc::clone(&self.app),
            ctx: ctx.clone(),
            guild,
            user,
            member,
            previous: None,
        };

        self.dispatch.member_remove(&cx).await;
    }

    async fn guild_member_update(
        &self,
        ctx: &Context,
        previous: Option<Member>,
        member: Option<Member>,
        event: &GuildMemberUpdateEvent,
    ) {
        let cx = MemberCx {
            app: Arc::clone(&self.app),
            ctx: ctx.clone(),
            guild: event.guild_id,
            user: event.user.clone(),
            member,
            previous,
        };

        self.dispatch.member_update(&cx).await;
    }

    async fn voice_state_update(
        &self,
        ctx: &Context,
        previous: Option<VoiceState>,
        current: VoiceState,
    ) {
        let Some(guild) = current.guild_id else {
            return;
        };

        let cx = VoiceCx {
            app: Arc::clone(&self.app),
            ctx: ctx.clone(),
            guild,
            user: current.user_id,
            bot: current
                .member
                .as_ref()
                .is_some_and(|member| member.user.bot()),
            previous,
            current,
        };

        self.dispatch.voice_state(&cx).await;
    }

    async fn interaction_create(&self, ctx: &Context, interaction: Interaction) {
        match interaction {
            Interaction::Component(component) => {
                interact::dispatch(Arc::clone(&self.app), ctx.clone(), component).await
            }
            Interaction::Modal(modal) => {
                interact::submitted(Arc::clone(&self.app), ctx.clone(), modal).await
            }
            Interaction::Command(command) => {
                #[cfg(feature = "web")]
                if command.data.name == "configure" {
                    return crate::web::entrypoint::launched(ctx, &command).await;
                }

                slash::run::handle_command(Arc::clone(&self.app), ctx.clone(), command).await
            }
            _ => (),
        }
    }

    async fn guild_audit_log_entry_create(
        &self,
        ctx: &Context,
        entry: AuditLogEntry,
        guild: GuildId,
    ) {
        audit::record(&self.app, ctx, entry, guild).await;
    }

    async fn message_update(&self, ctx: &Context, message: Message) {
        if message.author.bot() {
            return;
        }

        let message = Arc::new(message);
        let cx = MessageCx {
            app: Arc::clone(&self.app),
            ctx: ctx.clone(),
            msg: Arc::clone(&message),
        };

        self.dispatch.message_edit(&cx).await;
        amend::reconsider(Arc::clone(&self.app), ctx.clone(), message).await;
    }
}

pub async fn build(
    app: Arc<App>,
    dispatch: Dispatch,
    token: &str,
) -> Result<Client, Box<serenity::Error>> {
    let mut cache = Settings::default();

    cache.max_messages = 0;

    let token: Token = token
        .parse()
        .map_err(|failure| Box::new(serenity::Error::from(failure)))?;

    let http = HttpBuilder::new(token.clone())
        .client(crate::platform::http::discord())
        .build();

    ClientBuilder::new_with_http(
        token,
        Arc::new(http),
        GatewayIntents::non_privileged()
            .union(GatewayIntents::GUILD_MEMBERS)
            .union(GatewayIntents::MESSAGE_CONTENT),
    )
    .event_handler(Arc::new(Gateway {
        app,
        dispatch: Arc::new(dispatch),
        booted: AtomicBool::new(false),
    }))
    .cache_settings(cache)
    .await
    .map_err(Box::new)
}
