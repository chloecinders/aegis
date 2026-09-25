use serenity::all::User;

use crate::command::args::Arg;
use crate::command::cx::Cx;
use crate::command::error::{Error, Result};
use crate::command::{Command, Meta, Response};
use crate::features::archive::transcript;
use crate::platform::ui::embed::Embed;
use crate::platform::ui::tone::Tone;
use aegis_macros::{command, meta};

#[command]
pub struct MessageLog {
    #[arg(reply)]
    target: Arg<User>,
}

impl Command for MessageLog {
    const META: Meta = meta! {
        name: "messagelog",
        aliases: ["message_log", "msglog"],
        short: "Links a full message log of a members messages",
        full: "Shows you a full message log of a members messages, which includes deleted or automoderated messages. \
        You must authorize your Discord account when accessing the log to prevent unauthorized users from accessing messages.",
        category: Records,
        one_of: [MODERATE_MEMBERS, KICK_MEMBERS, BAN_MEMBERS, MANAGE_NICKNAMES],
        edit: Rerun,
    };

    async fn run(self, cx: &mut Cx) -> Result<Response> {
        let guild = cx.guild_snowflake()?;
        let target = self.target.into_value();

        let asked = transcript::Request::history(
            guild,
            target.id.get(),
            target.name.to_string(),
            cx.guild_name().await,
        );

        let Some(id) = transcript::store::build(cx.pool(), &asked).await? else {
            return Err(Error::empty().title("no stored messages found"));
        };

        let Some(link) = transcript::url(cx.app.config.web_url.as_deref(), guild, &id) else {
            return Err(Error::empty().title("built without web features"));
        };

        Ok(Response::embed(
            Embed::new("MESSAGE LOG")
                .body(format!("[view message log]({link})"))
                .tone(Tone::Info),
        ))
    }
}
