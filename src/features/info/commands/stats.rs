use chrono::Duration;
use sysinfo::{Pid, ProcessRefreshKind, ProcessesToUpdate, System};

use crate::command::cx::Cx;
use crate::command::error::Result;
use crate::command::{Command, Meta, Response};
use crate::platform::text::duration::precise;
use crate::platform::ui::embed::Embed;
use aegis_macros::{command, meta};

pub fn resident_mib() -> u64 {
    let mut system = System::new();
    let process_id = Pid::from_u32(std::process::id());

    system.refresh_processes_specifics(
        ProcessesToUpdate::Some(&[process_id]),
        true,
        ProcessRefreshKind::nothing().with_memory(),
    );

    system
        .process(process_id)
        .map(|process| process.memory() / (1024 * 1024))
        .unwrap_or_default()
}

#[command]
pub struct Stats {}

impl Command for Stats {
    const META: Meta = meta! {
        name: "stats",
        short: "Gets various bot statistics",
        full: "Shows various statistics of the bot. Useful for nerds!",
        category: Misc,
        edit: Rerun,
    };

    async fn run(self, cx: &mut Cx) -> Result<Response> {
        let uptime = Duration::from_std(cx.app.uptime()).unwrap_or_else(|_| Duration::zero());
        let guilds = cx.ctx.cache.guild_count();

        Ok(Response::embed(Embed::new("STATS").body(format!(
            "Servers: `{guilds}`\nUptime: `{}`\nMemory: `{} MiB`",
            precise(uptime),
            resident_mib()
        ))))
    }
}
