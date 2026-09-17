use serenity::all::User;

use crate::command::args::Arg;
use crate::command::cx::Cx;
use crate::command::error::Result;
use crate::command::{Command, Meta, Response};
use crate::domain::punishment::{Punishment, PunishmentType};
use crate::domain::reason::{Note, Reason};
use crate::features::punishments::executor::{self, Reply, Subject};
use crate::features::references::Reference;
use aegis_macros::{command, meta};

#[command]
pub struct Unban {
    #[arg(reply)]
    target: Arg<User>,
    #[arg(rest, amend = Reason)]
    reason: Reason,
    #[flag(short = 's', desc = "Skips DMing the target the reason")]
    silent: bool,
    #[flag(short = 'n', amend = Note, desc = "Attaches a note, which is not displayed in DMs")]
    note: Option<Note>,
    #[flag(
        name = "ref",
        short = 'r',
        desc = "Saves a message link or image as evidence"
    )]
    reference: Option<Reference>,
}

impl Command for Unban {
    const META: Meta = meta! {
        name: "unban",
        short: "Lifts a users ban",
        full: "Removes a ban and records the reversal against the users log. \
        Attaching an images saves the image as a reference.",
        category: Moderation,
        user: [BAN_MEMBERS],
        bot: [BAN_MEMBERS],
        edit: Amendable,
    };

    async fn run(self, cx: &mut Cx) -> Result<Response> {
        let reply = match self.target.was_inferred() {
            true => Reply::Temporary,
            false => Reply::Permanent,
        };
        let user = self.target.into_value();
        let punishment = Punishment::new(
            PunishmentType::Unban,
            cx.guild_snowflake()?,
            cx.author_id().get(),
            user.id.get(),
        )
        .reason(self.reason)
        .note(self.note)
        .silent(self.silent);

        executor::apply(
            cx,
            punishment,
            Subject::Absent(Box::new(user)),
            reply,
            self.reference,
            false,
        )
        .await
    }
}
