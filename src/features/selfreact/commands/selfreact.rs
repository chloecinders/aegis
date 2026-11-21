use crate::command::cx::Cx;
use crate::command::error::Result;
use crate::command::{Command, Meta, Response};
use crate::features::selfreact::store;
use crate::platform::ui::embed::Embed;
use crate::platform::ui::tone::Tone;
use aegis_macros::{command, meta};

#[command]
pub struct SelfReact {
    #[arg]
    enabled: bool,
}

impl Command for SelfReact {
    const META: Meta = meta! {
        name: "selfreact",
        short: "Removes selfreactions",
        full: "Feature to automatically remove selfreactions, because some people just can't fathom being funny.",
        category: Admin,
        user: [MANAGE_GUILD],
        edit: Rerun,
    };

    async fn run(self, cx: &mut Cx) -> Result<Response> {
        let guild = cx.guild_snowflake()?;

        store::set(cx.pool(), guild, !self.enabled).await?;

        cx.app.self_reactions.forget(guild);

        let title = match self.enabled {
            true => "SELF REACTIONS ALLOWED",
            false => "SELF REACTIONS BLOCKED",
        };

        Ok(Response::embed(Embed::new(title).tone(Tone::Success)))
    }
}
