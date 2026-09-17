use chrono::Duration;
use sysinfo::{Pid, ProcessRefreshKind, ProcessesToUpdate, System};

use crate::command::cx::Cx;
use crate::command::error::Result;
use crate::command::{Command, Meta, Response};
use crate::platform::text::duration::precise;
use crate::platform::ui::embed::Embed;
use aegis_macros::{command, meta};

#[cfg(not(target_env = "msvc"))]
pub fn heap_mib() -> Option<u64> {
    use tikv_jemalloc_ctl::{epoch, stats};

    let refresh = epoch::mib().ok()?;
    let allocated = stats::allocated::mib().ok()?;

    refresh.advance().ok()?;

    Some(allocated.read().ok()? as u64 / (1024 * 1024))
}

#[cfg(target_env = "msvc")]
pub fn heap_mib() -> Option<u64> {
    None
}

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

        let heap = match heap_mib() {
            Some(allocated) => format!("\nHeap: `{allocated} MiB`"),
            None => String::new(),
        };

        Ok(Response::embed(Embed::new("STATS").body(format!(
            "Servers: `{guilds}`\nUptime: `{}`\nMemory: `{} MiB`{heap}",
            precise(uptime),
            resident_mib()
        ))))
    }
}
