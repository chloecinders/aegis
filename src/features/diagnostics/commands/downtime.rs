use chrono::{Duration, Utc};

use crate::command::cx::Cx;
use crate::command::error::Result;
use crate::command::{Command, Meta, Response};
use crate::domain::logtype::LogType;
use crate::features::settings::store;
use crate::platform::ui::embed::Embed;
use crate::platform::ui::reply;
use crate::platform::ui::tone::Tone;
use aegis_macros::{command, meta};

#[command]
pub struct ScheduleDowntime {
    #[arg]
    starts_in: Duration,
    #[arg(rest)]
    detail: String,
}

pub fn notice(at: chrono::DateTime<Utc>, detail: &str) -> Embed {
    let body = match detail.trim().is_empty() {
        true => String::from("no further detail was given"),
        false => String::from(detail.trim()),
    };

    Embed::new("SCHEDULED DOWNTIME")
        .subtitle(format!("Starts: <t:{0}:R> (<t:{0}:f>)", at.timestamp()))
        .body(body)
        .tone(Tone::Warn)
}

impl Command for ScheduleDowntime {
    const META: Meta = meta! {
        name: "scheduledowntime",
        aliases: ["downtime"],
        short: "Announces planned downtime",
        full: "Posts a downtime notice to the announcements channel of every guild that has configured one. Takes how long until the downtime begins, then what to tell people.",
        category: Developer,
        developer: true,
        edit: Fixed,
    };

    async fn run(self, cx: &mut Cx) -> Result<Response> {
        let at = Utc::now() + self.starts_in;
        let announcement = notice(at, &self.detail);
        let routed = store::everywhere(cx.pool(), LogType::AegisAnnouncements).await?;

        for (_, channel) in &routed {
            let _ = channel
                .widen()
                .send_message(&cx.ctx.http, reply::plain(&announcement))
                .await;
        }

        Ok(Response::embed(
            Embed::new("DOWNTIME")
                .subtitle(format!("Starts: <t:{}:R>", at.timestamp()))
                .tone(Tone::Info),
        ))
    }
}
