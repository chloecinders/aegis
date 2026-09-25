use crate::command::args::Arg;
use crate::command::cx::Cx;
use crate::command::error::{Error, Result};
use crate::command::{Command, Meta, Response};
use crate::domain::ids::ActionId;
use crate::domain::reason::Note;
use crate::features::records::{answer, refreshed, store, ui};
use aegis_macros::{command, meta};

#[command]
pub struct SetNote {
    #[arg(reply)]
    id: Arg<ActionId>,
    #[arg(rest, amend = Note)]
    note: Option<Note>,
}

impl Command for SetNote {
    const META: Meta = meta! {
        name: "note",
        short: "Sets or clears the moderator note on an action",
        full: "Attaches a private note to an action. Unlike reasons, notes are never sent to the target. \
        Passing clear, or nothing, removes it.",
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

        store::set_note(cx.pool(), guild, &id, self.note.as_ref()).await?;

        let after = crate::domain::action::Action {
            note: self.note.clone(),
            ..before
        };

        if let Err(failure) = refreshed(cx.pool(), &cx.ctx, &after).await {
            cx.report(&failure);
        }

        let entry = match &self.note {
            None => ui::cleared(&after, "note"),
            Some(note) => ui::amended(&after, "note", note.as_str()),
        };

        answer(cx, entry, replied).await
    }
}
