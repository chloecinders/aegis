use serenity::all::Member;

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
pub struct Softban {
    #[arg(reply)]
    target: Arg<Member>,
    #[arg(rest, amend = Reason)]
    reason: Reason,
    #[flag(
        name = "clear",
        short = 'c',
        desc = "Days of the target's messages to delete"
    )]
    clear_days: Option<u8>,
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

impl Command for Softban {
    const META: Meta = meta! {
        name: "softban",
        short: "Kicks a member and clears their recent messages",
        full: "Bans and immediately unbans a member, which removes them and deletes their recent messages. \
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
        let member = self.target.into_value();
        let punishment = Punishment::new(
            PunishmentType::Softban,
            cx.guild_snowflake()?,
            cx.author_id().get(),
            member.user.id.get(),
        )
        .reason(self.reason)
        .note(self.note)
        .clear_days(self.clear_days.unwrap_or(1))
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
