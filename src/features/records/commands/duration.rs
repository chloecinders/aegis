use chrono::Duration;

use crate::command::args::Arg;
use crate::command::cx::Cx;
use crate::command::edit::Change;
use crate::command::error::{Error, Result};
use crate::command::{Command, Meta, Response};
use crate::domain::action::Amendment;
use crate::domain::ids::ActionId;
use crate::features::records::{amend, answer, store};
use aegis_macros::{command, meta};

#[command]
pub struct SetDuration {
    #[arg(reply)]
    id: Arg<ActionId>,
    #[arg(amend = Duration)]
    duration: Duration,
}

impl Command for SetDuration {
    const META: Meta = meta! {
        name: "duration",
        short: "Edits the duration of a punishment",
        full: "Edits the duration of a punishment. The new duration is calculated from when the duration command is ran, \
        not from when the punishment started. For example if you ban someone for 1 week, wait a day then change the \
        duration to 5d the member will stay banned for 6 days total, from the time the punishment happened. \
        Passing 0 will make the duration permanent.",
        category: Records,
        one_of: [MODERATE_MEMBERS, KICK_MEMBERS, BAN_MEMBERS, MANAGE_NICKNAMES],
        edit: Amendable,
    };

    async fn run(self, cx: &mut Cx) -> Result<Response> {
        let guild = cx.guild_snowflake()?;
        let replied = self.id.was_inferred();
        let id = self.id.into_value();

        let Some(action) = store::load(cx.pool(), guild, &id).await? else {
            return Err(Error::empty().title("log not found"));
        };

        if !action.verb.has_duration() {
            return Err(Error::empty().title("only bans and mutes have durations"));
        }

        if !action.state.active() {
            return Err(Error::empty().title("action no longer active"));
        }

        let change = Change {
            field: "duration",
            policy: Amendment::Duration,
            before: serde_json::Value::from(action.duration().num_seconds()),
            after: serde_json::Value::from(self.duration.num_seconds()),
        };

        let updated = amend::apply(cx, &action, std::slice::from_ref(&change)).await?;

        answer(cx, amend::rendered(&updated, &change), replied).await
    }
}
