use crate::command::args::Arg;
use crate::command::cx::Cx;
use crate::command::error::{Error, Result};
use crate::command::{Command, Meta, Response};
use crate::domain::ids::ActionId;
use crate::features::records::store;
use crate::features::references::{self, Reference};
use aegis_macros::{command, meta};

#[command]
pub struct EditRef {
    #[arg(reply)]
    id: Arg<ActionId>,
    #[arg]
    reference: Reference,
}

impl Command for EditRef {
    const META: Meta = meta! {
        name: "editref",
        aliases: ["edit_ref"],
        short: "Replaces the reference of an action",
        full: "Replaces the reference of an action. If images are provided on the command the images will be saved. \
        If a message link is provided the linked messages content will be saved. Note that the message must be inside \
        of the current server.",
        category: Records,
        one_of: [MODERATE_MEMBERS, KICK_MEMBERS, BAN_MEMBERS, MANAGE_NICKNAMES],
        edit: Rerun,
    };

    async fn run(self, cx: &mut Cx) -> Result<Response> {
        let guild = cx.guild_snowflake()?;
        let id = self.id.into_value();

        let Some(action) = store::load(cx.pool(), guild, &id).await? else {
            return Err(Error::empty().title("log not found"));
        };

        let Some(captured) = references::capture(cx, Some(self.reference)).await else {
            return Err(Error::empty().title("message unreadable"));
        };

        references::store::save(cx.pool(), &action.id, &captured).await?;

        if let Err(failure) = references::controls::attach(cx.pool(), &cx.ctx, &action).await {
            cx.report(&failure);
        }

        Ok(Response::embed(references::ui::viewed(guild, &captured)))
    }
}
