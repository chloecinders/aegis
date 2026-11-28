use std::sync::Arc;
use std::time::Duration;

use serenity::all::{Context, Message, MessageId, UserId};
use tokio::task::JoinHandle;
use tokio::time::{sleep, timeout};

use crate::command::cx::Cx;
use crate::command::error::{Ctx, Result};
use crate::domain::punishment::DmTiming;
use crate::platform::ui::embed::Embed;
use crate::platform::ui::marks::Marks;
use crate::platform::ui::reply;

pub type NoticeCallback = Arc<dyn Fn(&Message) + Send + Sync>;

pub struct Delivery {
    target: UserId,
    timing: DmTiming,
    silent: bool,
    auto_delete: bool,
    notice: Option<Embed>,
    on_notice: Option<NoticeCallback>,
    inflight: Option<JoinHandle<bool>>,
    outcome: Option<bool>,
}

impl Delivery {
    pub fn new(target: UserId, timing: DmTiming) -> Self {
        Self {
            target,
            timing,
            silent: false,
            auto_delete: false,
            notice: None,
            on_notice: None,
            inflight: None,
            outcome: None,
        }
    }

    pub fn notice(mut self, embed: Embed) -> Self {
        self.notice = Some(embed);
        self
    }

    pub fn on_notice(mut self, on_notice: NoticeCallback) -> Self {
        self.on_notice = Some(on_notice);
        self
    }

    pub fn silent(mut self, silent: bool) -> Self {
        self.silent = silent;
        self
    }

    pub fn auto_delete(mut self, auto_delete: bool) -> Self {
        self.auto_delete = auto_delete;
        self
    }

    pub fn skips_dm(&self) -> bool {
        self.silent || self.timing == DmTiming::Never || self.notice.is_none()
    }

    pub async fn notify(&mut self, cx: &Cx) {
        if self.skips_dm() {
            return;
        }

        let Some(embed) = self.notice.take() else {
            return;
        };

        if self.timing == DmTiming::Before {
            let opened = self.target.create_dm_channel(&cx.ctx).await;

            cx.trace("open_dm_channel");

            let sent = match opened {
                Ok(channel) => {
                    channel
                        .id
                        .widen()
                        .send_message(&cx.ctx.http, reply::plain(&embed))
                        .await
                }
                Err(failure) => Err(failure),
            };

            if let (Ok(notice), Some(on_notice)) = (&sent, &self.on_notice) {
                on_notice(notice);
            }

            self.outcome = Some(sent.is_ok());

            return;
        }

        let http = cx.ctx.clone();
        let target = self.target;
        let on_notice = self.on_notice.clone();
        let sending = tokio::spawn(async move {
            let sent = target.direct_message(&http, reply::plain(&embed)).await;

            if let (Ok(notice), Some(on_notice)) = (&sent, &on_notice) {
                on_notice(notice);
            }

            sent.is_ok()
        });

        self.inflight = Some(sending);
    }

    pub async fn delivered(&mut self) -> Option<bool> {
        if self.skips_dm() && self.outcome.is_none() {
            return None;
        }

        let Some(handle) = self.inflight.take() else {
            return self.outcome;
        };

        let grace = Duration::from_millis(300);
        let outcome = match timeout(grace, handle).await {
            Ok(joined) => joined.unwrap_or(false),
            Err(_) => true,
        };

        self.outcome = Some(outcome);

        self.outcome
    }

    pub fn marks(&self) -> Marks {
        Marks {
            silent: self.silent,
            dm_failed: self.outcome == Some(false),
            ..Default::default()
        }
    }

    pub async fn respond(
        &mut self,
        ctx: &Context,
        original: &Message,
        render: impl Fn(Marks) -> Embed,
    ) -> Result<MessageId> {
        self.delivered().await;

        let embed = render(self.marks());
        let posted = original
            .channel_id
            .send_message(&ctx.http, reply::plain(&embed).reference_message(original))
            .await
            .ctx("send command response")?;

        if self.auto_delete {
            self.sweep(ctx, original, &posted);
        }

        Ok(posted.id)
    }

    fn sweep(&self, ctx: &Context, original: &Message, posted: &Message) {
        let ctx = ctx.clone();
        let original = original.clone();
        let posted = posted.clone();

        tokio::spawn(async move {
            sleep(Duration::from_secs(5)).await;

            let _ = tokio::join!(
                original.delete(&ctx.http, None),
                posted.delete(&ctx.http, None)
            );
        });
    }
}
