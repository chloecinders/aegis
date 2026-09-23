use std::time::Instant;

use crate::command::cx::Cx;
use crate::command::error::{Ctx as _, Result};
use crate::command::{Command, Meta, Response};
use crate::platform::ui::embed::Embed;
use aegis_macros::{command, meta};

#[command]
pub struct Ping {}

impl Command for Ping {
    const META: Meta = meta! {
        name: "ping",
        short: "Gets the bots current latency",
        full: "Gets the bots HTTP and gateway latency. Useful for checking if the bot is lagging.",
        category: Misc,
        edit: Rerun,
    };

    async fn run(self, cx: &mut Cx) -> Result<Response> {
        cx.trace("testing_http_latency");

        let http = {
            let start = Instant::now();

            cx.ctx
                .http
                .get_current_user()
                .await
                .ctx("measure http latency")?;

            start.elapsed().as_millis()
        };

        cx.trace("testing_gateway_latency");

        let gateway = cx
            .app
            .shard_latency(cx.ctx.shard_id)
            .map(|round| format!("`{}ms`", round.as_millis()))
            .unwrap_or_else(|| String::from("`unknown`"));

        Ok(Response::embed(
            Embed::new("PING")
                .subtitle(format!("Shard: {}", cx.ctx.shard_id.0))
                .body(format!("HTTP: `{http}ms`\nGateway: {gateway}")),
        ))
    }
}
