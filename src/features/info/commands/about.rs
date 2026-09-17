use chrono::Duration;

use crate::command::cx::Cx;
use crate::command::error::Result;
use crate::command::{Command, Meta, Response};
use crate::features::info::commands::stats::resident_mib;
use crate::platform::text::duration::precise;
use crate::platform::ui::embed::Embed;
use aegis_macros::{command, meta};

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

#[command]
pub struct About {}

impl Command for About {
    const META: Meta = meta! {
        name: "about",
        short: "Gets general information about the bot",
        full: "Gets general information about the bot, plus credits and important links.",
        category: Misc,
        edit: Rerun,
    };

    async fn run(self, cx: &mut Cx) -> Result<Response> {
        let name = cx.ctx.cache.current_user().name.clone();
        let prefix = cx.app.prefix();
        let uptime = Duration::from_std(cx.app.uptime()).unwrap_or_else(|_| Duration::zero());
        let guilds = cx.ctx.cache.guild_count();

        let body = format!(
            "Hey, I'm {name}!
A moderation bot made for one purpose and one purpose only: Moderation.
Visit <https://aegis.chloecinders.com/> for more information.
I'm currently in private beta but my source code is available at <https://github.com/chloecinders/aegis>.
Type `{prefix}help` to see a list of all commands!

I was made in Rust by chloecinders!

Special thanks to:
```
serenity-rs: Underlying Bot Framework
andreashgk: Rust Mentorship
Discord Previews & Rust Central: Bots pre-release testing grounds
```
Nerd Stats:
Version: {VERSION}
Servers: {guilds}
Uptime: {}
Memory: {} MiB

[`privacy policy`](https://aegis.chloecinders.com/privacy/) [`terms of service`](https://aegis.chloecinders.com/terms/)",
            precise(uptime),
            resident_mib()
        );

        Ok(Response::embed(Embed::new("ABOUT").body(body)))
    }
}
