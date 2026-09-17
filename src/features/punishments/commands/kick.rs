use crate::command::args::Arg;
use crate::command::cx::Cx;
use crate::command::error::Result;
use crate::command::{Command, Meta, Response};
use crate::domain::punishment::{Punishment, PunishmentType};
use crate::domain::reason::{Note, Reason};
use crate::features::punishments::executor::{self, Reply, Subject};
use crate::features::references::Reference;
use aegis_macros::{command, meta};
use serenity::all::Member;

#[command]
pub struct Kick {
    #[arg(reply)]
    target: Arg<Member>,
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
    #[flag(
        short = 'k',
        desc = "Keeps the replied to message instead of deleting it"
    )]
    keep: bool,
}

impl Command for Kick {
    const META: Meta = meta! {
        name: "kick",
        short: "Kicks a member from the server",
        full: "Removes a member from the server. Attaching an images saves the image as a reference.",
        category: Moderation,
        user: [KICK_MEMBERS],
        bot: [KICK_MEMBERS],
        edit: Amendable,
    };

    async fn run(self, cx: &mut Cx) -> Result<Response> {
        let reply = match self.target.was_inferred() {
            true => Reply::Temporary,
            false => Reply::Permanent,
        };
        let member = self.target.into_value();
        let punishment = Punishment::new(
            PunishmentType::Kick,
            cx.guild_snowflake()?,
            cx.author_id().get(),
            member.user.id.get(),
        )
        .reason(self.reason)
        .note(self.note)
        .silent(self.silent);

        executor::apply(
            cx,
            punishment,
            Subject::Present(Box::new(member)),
            reply,
            self.reference,
            self.keep,
        )
        .await
    }
}
