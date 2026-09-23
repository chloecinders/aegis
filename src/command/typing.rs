use std::time::Duration;

use serenity::all::{Context, GenericChannelId};
use tokio::task::JoinHandle;
use tokio::time::sleep;

pub struct Typing {
    signal: JoinHandle<()>,
}

impl Typing {
    pub fn watch(ctx: &Context, channel: GenericChannelId) -> Self {
        let http = ctx.http.clone();

        Self {
            signal: tokio::spawn(async move {
                sleep(Duration::from_millis(500)).await;

                loop {
                    let _ = channel.broadcast_typing(&http).await;

                    sleep(Duration::from_secs(8)).await;
                }
            }),
        }
    }
}

impl Drop for Typing {
    fn drop(&mut self) {
        self.signal.abort();
    }
}
