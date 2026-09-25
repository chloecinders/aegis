use crate::command::args::Arg;
use crate::command::cx::Cx;
use crate::command::error::{Error, Result};
use crate::command::{Command, Meta, Response};
use crate::domain::ids::ActionId;
use crate::features::references::{self, store, ui};
use aegis_macros::{command, meta};

#[command]
pub struct ViewRef {
    #[arg(reply)]
    id: Arg<ActionId>,
}

impl Command for ViewRef {
    const META: Meta = meta! {
        name: "ref",
        short: "Shows the reference of an action",
        full: "Shows the refrence of an action.",
        category: Records,
        one_of: [MODERATE_MEMBERS, KICK_MEMBERS, BAN_MEMBERS, MANAGE_NICKNAMES],
        edit: Rerun,
    };

    async fn run(self, cx: &mut Cx) -> Result<Response> {
        let id = self.id.into_value();
        let guild = cx.guild_snowflake()?;

        let Some(mut captured) = store::load(cx.pool(), guild, &id).await? else {
            return Err(Error::empty().title("log not found"));
        };

        references::confirm(cx.pool(), &cx.ctx, &id, &mut captured).await?;

        Ok(Response::embed(ui::viewed(guild, &captured)))
    }
}
