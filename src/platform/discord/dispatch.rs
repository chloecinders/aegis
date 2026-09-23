use std::sync::Arc;

use futures::future::join_all;
use serenity::all::{
    Context, GenericChannelId, GuildId, Member, Message, MessageId, User, UserId, VoiceState,
};
use serenity::async_trait;

use crate::app::App;

pub struct MessageCx {
    pub app: Arc<App>,
    pub ctx: Context,
    pub msg: Arc<Message>,
}

pub struct MemberCx {
    pub app: Arc<App>,
    pub ctx: Context,
    pub guild: GuildId,
    pub user: User,
    pub member: Option<Member>,
    pub previous: Option<Member>,
}

pub struct DeletionCx {
    pub app: Arc<App>,
    pub ctx: Context,
    pub guild: Option<GuildId>,
    pub channel: GenericChannelId,
    pub message: MessageId,
}

pub struct BulkDeletionCx {
    pub app: Arc<App>,
    pub ctx: Context,
    pub guild: Option<GuildId>,
    pub channel: GenericChannelId,
    pub messages: Vec<MessageId>,
}

pub struct VoiceCx {
    pub app: Arc<App>,
    pub ctx: Context,
    pub guild: GuildId,
    pub user: UserId,
    pub bot: bool,
    pub previous: Option<VoiceState>,
    pub current: VoiceState,
}

#[async_trait]
pub trait Observer: Send + Sync {
    fn name(&self) -> &'static str;

    async fn on_message(&self, _cx: &MessageCx) {}

    async fn on_message_edit(&self, _cx: &MessageCx) {}

    async fn on_message_delete(&self, _cx: &DeletionCx) {}

    async fn on_message_delete_bulk(&self, _cx: &BulkDeletionCx) {}

    async fn on_member_add(&self, _cx: &MemberCx) {}

    async fn on_member_remove(&self, _cx: &MemberCx) {}

    async fn on_member_update(&self, _cx: &MemberCx) {}

    async fn on_voice_state(&self, _cx: &VoiceCx) {}
}

#[derive(Default)]
pub struct Dispatch {
    observers: Vec<Arc<dyn Observer>>,
}

impl Dispatch {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add(&mut self, observer: Arc<dyn Observer>) {
        self.observers.push(observer);
    }

    pub fn names(&self) -> Vec<&'static str> {
        self.observers
            .iter()
            .map(|observer| observer.name())
            .collect()
    }

    pub async fn message(&self, cx: &MessageCx) {
        join_all(
            self.observers
                .iter()
                .map(|observer| observer.on_message(cx)),
        )
        .await;
    }

    pub async fn message_edit(&self, cx: &MessageCx) {
        join_all(
            self.observers
                .iter()
                .map(|observer| observer.on_message_edit(cx)),
        )
        .await;
    }

    pub async fn message_delete(&self, cx: &DeletionCx) {
        join_all(
            self.observers
                .iter()
                .map(|observer| observer.on_message_delete(cx)),
        )
        .await;
    }

    pub async fn message_delete_bulk(&self, cx: &BulkDeletionCx) {
        join_all(
            self.observers
                .iter()
                .map(|observer| observer.on_message_delete_bulk(cx)),
        )
        .await;
    }

    pub async fn member_add(&self, cx: &MemberCx) {
        for observer in &self.observers {
            observer.on_member_add(cx).await;
        }
    }

    pub async fn member_remove(&self, cx: &MemberCx) {
        for observer in &self.observers {
            observer.on_member_remove(cx).await;
        }
    }

    pub async fn member_update(&self, cx: &MemberCx) {
        join_all(
            self.observers
                .iter()
                .map(|observer| observer.on_member_update(cx)),
        )
        .await;
    }

    pub async fn voice_state(&self, cx: &VoiceCx) {
        join_all(
            self.observers
                .iter()
                .map(|observer| observer.on_voice_state(cx)),
        )
        .await;
    }
}
