use chrono::Duration;
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
pub struct Mute {
    #[arg(reply)]
    target: Arg<Member>,
    #[arg(amend = Duration)]
    duration: Option<Duration>,
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

impl Command for Mute {
    const META: Meta = meta! {
        name: "mute",
        aliases: ["timeout"],
        short: "Times a member out",
        full: "Times a member out for the given duration. Leave the duration out, or pass 0, for a mute with no end date. \
        As Discord timeouts are limited to 30 days, anything longer will be handled as a 'managed mute'. \
        This means that the bot will automatically re-mute the person until the specified duration is reached. \
        Please avoid unmuting members manually, as this will cause them to automatically get muted again.",
        category: Moderation,
        user: [MODERATE_MEMBERS],
        bot: [MODERATE_MEMBERS],
        edit: Amendable,
    };

    async fn run(self, cx: &mut Cx) -> Result<Response> {
        let reply = match self.target.was_inferred() {
            true => Reply::Temporary,
            false => Reply::Permanent,
        };
        let member = self.target.into_value();
        let punishment = Punishment::new(
            PunishmentType::Mute,
            cx.guild_snowflake()?,
            cx.author_id().get(),
            member.user.id.get(),
        )
        .duration(self.duration.unwrap_or_else(Duration::zero))
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
