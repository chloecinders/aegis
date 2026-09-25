use serenity::all::Color;

use crate::command::cx::Cx;
use crate::command::error::{Error, Result};
use crate::command::{Command, Meta, Response};
use crate::features::sticky::store;
use crate::platform::ui::embed::Embed;
use crate::platform::ui::tone::Tone;
use aegis_macros::{command, meta};

#[command]
pub struct SetSticky {
    #[arg(rest)]
    content: Option<String>,
    #[flag(short = 't', desc = "Sets the title of the embed")]
    title: Option<String>,
    #[flag(short = 'c', desc = "Sets the color of the embed")]
    color: Option<Color>,
}

impl Command for SetSticky {
    const META: Meta = meta! {
        name: "sticky",
        short: "Create a sticky message",
        full: "Sticky messages are messages that get deleted and re-sent every time someone else sends another message. \
        The point is to keep a specific message viewable at the very bottom of the channel. \
        Providing either a title or a color will display the message using an embed.",
        category: Admin,
        user: [MANAGE_MESSAGES],
        edit: Rerun,
    };

    async fn run(self, cx: &mut Cx) -> Result<Response> {
        let guild = cx.guild_snowflake()?;
        let channel = cx.channel_id().get();

        let Some(content) = self.content else {
            if self.title.is_some() || self.color.is_some() {
                return Err(Error::new(cx.input())
                    .title("no message provided")
                    .with_all("write the message below"));
            }

            if !store::clear(cx.pool(), channel).await? {
                return Err(Error::empty().title("sticky not found"));
            }

            cx.app.stickies.forget(channel);

            return Ok(Response::embed(
                Embed::new("STICKY REMOVED").tone(Tone::Success),
            ));
        };

        store::set(
            cx.pool(),
            guild,
            channel,
            &content,
            self.title.as_deref(),
            self.color,
        )
        .await?;

        cx.app.stickies.forget(channel);

        Ok(Response::embed(
            Embed::new("STICKY SET").body(content).tone(Tone::Success),
        ))
    }
}
