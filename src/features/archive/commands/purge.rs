use chrono::{Duration, Utc};
use serenity::all::{GetMessages, Member, MessageId};

use crate::command::cx::Cx;
use crate::command::error::{Ctx, Error, Result};
use crate::command::{Command, Meta, Response};
use crate::domain::Snowflake;
use crate::domain::logtype::LogType;
use crate::features::archive::store;
#[cfg(feature = "web")]
use crate::features::archive::transcript;
use crate::features::guildlog;
use crate::platform::text::truncate;
use crate::platform::ui::embed::{Embed, channel_mention, code, mention};
use crate::platform::ui::tone::Tone;
use aegis_macros::{command, meta};

#[command]
pub struct Purge {
    #[arg]
    count: u8,
    #[flag(short = 'u', desc = "Only delete messages from this member")]
    user: Option<Member>,
    #[flag(short = 'c', desc = "Only delete messages containing this text")]
    contains: Option<String>,
}

impl Command for Purge {
    const META: Meta = meta! {
        name: "purge",
        aliases: ["clear", "prune"],
        short: "Bulk deletes recent messages",
        full: "Bulk deletes up to 100 recent messages in this channel, optionally only \
        those from one member or only those containing some text. Discord can not \
        bulk delete anything older than two weeks. Removed messages are saved into a transcript which is linked in the message log. \
        You must authorize your Discord account when accessing the transcript to prevent unauthorized users from accessing messages.",
        category: Moderation,
        user: [MANAGE_MESSAGES],
        bot: [MANAGE_MESSAGES],
        edit: Fixed,
    };

    async fn run(self, cx: &mut Cx) -> Result<Response> {
        let guild = cx.guild_snowflake()?;
        let channel = cx.channel_id();
        let wanted = self.count.min(100);

        let recent = channel
            .messages(
                &cx.ctx.http,
                GetMessages::new().before(cx.msg.id).limit(100),
            )
            .await
            .ctx("read messages to purge")?;

        let cutoff = Utc::now() - Duration::days(14);
        let subject = self.user.as_ref().map(|member| member.user.id);
        let needle = self.contains.as_ref().map(|text| text.to_lowercase());
        let doomed: Vec<MessageId> = recent
            .into_iter()
            .filter(|message| subject.is_none_or(|wanted| message.author.id == wanted))
            .filter(|message| {
                needle
                    .as_ref()
                    .is_none_or(|text| message.content.to_lowercase().contains(text))
            })
            .filter(|message| *message.timestamp >= cutoff)
            .map(|message| message.id)
            .take(wanted as usize)
            .collect();

        if doomed.is_empty() {
            let filtered = subject.is_some() || needle.is_some();

            return Err(match filtered {
                true => Error::bare().title("no matches found in the last 100 messages"),
                false => Error::bare().title("no messages younger than 2 weeks found"),
            });
        }

        if doomed.len() < 2 {
            return Err(Error::bare().title("discord will not bulk delete fewer than 2"));
        }

        cx.app
            .pending
            .expect_deletions(channel.get(), doomed.iter().map(|id| id.get()));

        let ids: Vec<Snowflake> = doomed.iter().map(|id| id.get()).collect();

        channel
            .delete_messages(&cx.ctx.http, &doomed)
            .await
            .ctx("purge messages")?;

        if let Err(failure) = store::removed_many(cx.pool(), guild, &ids).await {
            cx.report(&failure);
        }

        let link = preserve(cx, guild, ids).await;
        let actor = cx.author_id().get();

        let entry = Embed::new("MESSAGES PURGED")
            .subtitle(format!("Channel: {}", channel_mention(channel.get())))
            .subtitle(format!("Removed: {}", doomed.len()))
            .subtitle(format!(
                "Actor: {}",
                mention(actor, Some(&cx.msg.author.name))
            ))
            .maybe_subtitle(self.user.as_ref().map(|member| {
                format!(
                    "From: {}",
                    mention(member.user.id.get(), Some(&member.user.name))
                )
            }))
            .maybe_subtitle(
                self.contains
                    .as_deref()
                    .map(|text| format!("Containing: {}", code(&truncate::clamp(text, 60)))),
            )
            .maybe_footnote(link.map(|link| format!("[View transcript]({link})")))
            .tone(Tone::Success);

        let logged = guildlog::emit(
            cx,
            LogType::MessageUpdate,
            &entry,
            guildlog::Subject {
                target: subject.map(|id| id.get()).unwrap_or(channel.get()),
                moderator: Some(actor),
                action: None,
            },
            &[],
        )
        .await;

        let posted = match logged {
            Ok(posted) => posted,
            Err(failure) => {
                cx.report(&failure);

                None
            }
        };

        if posted.is_none() {
            return Ok(Response::embed(entry));
        }

        cx.app.pending.silence(channel.get(), cx.msg.id.get());

        if let Err(failure) = cx.msg.delete(&cx.ctx).await.ctx("clear purge command") {
            cx.report(&failure);
        }

        Ok(Response::None)
    }
}

#[cfg(feature = "web")]
async fn preserve(cx: &Cx, guild: Snowflake, ids: Vec<Snowflake>) -> Option<String> {
    let asked = transcript::Request::selection(guild, ids, cx.guild_name().await);

    match transcript::store::build(cx.pool(), &asked).await {
        Ok(built) => {
            built.and_then(|id| transcript::url(cx.app.config.web_url.as_deref(), guild, &id))
        }
        Err(failure) => {
            cx.report(&failure);

            None
        }
    }
}

#[cfg(not(feature = "web"))]
async fn preserve(_cx: &Cx, _guild: Snowflake, _ids: Vec<Snowflake>) -> Option<String> {
    None
}
