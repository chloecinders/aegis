use chrono::Duration;
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
pub struct Ban {
    #[arg(reply)]
    target: Arg<User>,
    #[arg(amend = Duration)]
    duration: Option<Duration>,
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

impl Command for Ban {
    const META: Meta = meta! {
        name: "ban",
        short: "Bans a user from the server",
        full: "Bans a user from the server with a given duration, clearing recent messages. \
        Bans are permanent if no duration is given and 1 day of messages is cleared if the \
        clear flag isnt passed. Attaching an image saves the image as a reference.",
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
            PunishmentType::Ban,
            cx.guild_snowflake()?,
            cx.author_id().get(),
            user.id.get(),
        )
        .reason(self.reason)
        .note(self.note)
        .duration(self.duration.unwrap_or_else(Duration::zero))
        .clear_days(self.clear_days.unwrap_or(1))
        .silent(self.silent);

        let subject = match cx.member(user.id).await {
            Ok(member) => Subject::Present(Box::new(member)),
            Err(_) => Subject::Absent(Box::new(user)),
        };

        executor::apply(cx, punishment, subject, reply, self.reference, self.keep).await
    }
}
