use crate::command::args::Arg;
use crate::command::cx::Cx;
use crate::command::error::{Error, Result};
use crate::command::{Command, Meta, Response};
use crate::domain::ids::ActionId;
use crate::domain::reason::Reason;
use crate::features::records::{answer, refreshed, store, ui};
use aegis_macros::{command, meta};

#[command]
pub struct SetReason {
    #[arg(reply)]
    id: Arg<ActionId>,
    #[arg(rest, amend = Reason)]
    reason: Reason,
}

impl Command for SetReason {
    const META: Meta = meta! {
        name: "reason",
        short: "Updates the reason on a punishment",
        full: "Updates the reason on a punishment.",
        category: Records,
        one_of: [MODERATE_MEMBERS, KICK_MEMBERS, BAN_MEMBERS, MANAGE_NICKNAMES],
        edit: Amendable,
    };

    async fn run(self, cx: &mut Cx) -> Result<Response> {
        let guild = cx.guild_snowflake()?;
        let replied = self.id.was_inferred();
        let id = self.id.into_value();

        let Some(before) = store::load(cx.pool(), guild, &id).await? else {
            return Err(Error::empty().title("log not found"));
        };

        store::set_reason(cx.pool(), guild, &id, &self.reason).await?;

        let after = crate::domain::action::Action {
            reason: self.reason.clone(),
            ..before
        };

        if let Err(failure) = refreshed(cx.pool(), &cx.ctx, &after).await {
            cx.report(&failure);
        }

        answer(
            cx,
            ui::amended(&after, "reason", self.reason.as_str()),
            replied,
        )
        .await
    }
}
