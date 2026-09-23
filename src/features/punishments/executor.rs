use std::sync::Arc;

#[cfg(feature = "web")]
use chrono::{Duration, Utc};
use serenity::all::{EditMember, Member, Message, MessageType, Permissions, User, UserId};

use crate::command::Response;
use crate::command::cx::Cx;
use crate::command::error::{Ctx, Error, Result};
use crate::domain::logtype::LogType;
use crate::domain::punishment::{Punishment, PunishmentType};
#[cfg(feature = "web")]
use crate::features::archive::{self, transcript, transcript::store as transcripts};
use crate::features::guildlog;
use crate::features::punishments::scheduled::{self, Kind};
use crate::features::punishments::store;
use crate::features::records;
use crate::features::references::{self, Captured, Reference};
use crate::platform::discord::fetch;
use crate::platform::ui::delivery::Delivery;
use crate::platform::ui::marks::Marks;
use crate::platform::ui::punishment as ui;

pub enum Subject {
    Present(Box<Member>),
    Absent(Box<User>),
}

impl Subject {
    pub fn id(&self) -> UserId {
        match self {
            Subject::Present(member) => member.user.id,
            Subject::Absent(user) => user.id,
        }
    }

    pub fn name(&self) -> &str {
        match self {
            Subject::Present(member) => &member.user.name,
            Subject::Absent(user) => &user.name,
        }
    }

    pub fn member(&self) -> Option<&Member> {
        match self {
            Subject::Present(member) => Some(member),
            Subject::Absent(_) => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Reply {
    Permanent,
    Temporary,
    None,
}

pub fn authority(verb: PunishmentType) -> Permissions {
    match verb {
        PunishmentType::Ban | PunishmentType::Softban | PunishmentType::Unban => {
            Permissions::BAN_MEMBERS
        }
        PunishmentType::Kick => Permissions::KICK_MEMBERS,
        PunishmentType::Mute | PunishmentType::Unmute | PunishmentType::Warn => {
            Permissions::MODERATE_MEMBERS
        }
    }
}

pub async fn apply(
    cx: &mut Cx,
    mut punishment: Punishment,
    subject: Subject,
    reply: Reply,
    reference: Option<Reference>,
    keep_offending: bool,
) -> Result<Response> {
    if let Some(member) = subject.member() {
        if punishment.actor != cx.bot_id().get()
            && !cx.can_target(member, authority(punishment.verb)).await
        {
            return Err(Error::bare().title("cannot target this member"));
        }

        if punishment.verb != PunishmentType::Warn
            && !cx.bot_can_target(member, authority(punishment.verb)).await
        {
            return Err(Error::bare().title("bot cannot target this member"));
        }
    }

    cx.trace("target_check");

    store::supersede(cx.pool(), &punishment).await?;
    store::insert(cx.pool(), &mut punishment).await?;
    cx.note_action(punishment.id.clone());
    cx.trace("persist_action");

    let offending = cx
        .msg
        .referenced_message
        .as_deref()
        .filter(|replied| {
            reply == Reply::Temporary
                && replied.author.id.get() == punishment.target
                && replied.kind != MessageType::AutoModAction
                && !matches!(
                    punishment.verb,
                    PunishmentType::Unban | PunishmentType::Unmute
                )
        })
        .map(|replied| (replied.channel_id, replied.id));

    let reference = reference.or_else(|| {
        offending.map(|(channel, message)| Reference {
            guild: punishment.guild,
            channel: channel.get(),
            message: message.get(),
        })
    });

    let captured = references::capture(cx, reference).await;

    if let Some(captured) = &captured
        && let Err(failure) = references::store::save(cx.pool(), &punishment.id, captured).await
    {
        cx.report(&failure);
    }

    let guild_name = cx.guild_name().await;

    let invocation = cx.msg.id.get();
    let app = Arc::clone(&cx.app);

    let mut delivery = Delivery::new(subject.id(), punishment.verb.dm_timing())
        .notice(ui::notice(&punishment, &guild_name))
        .silent(punishment.silent)
        .auto_delete(reply == Reply::Temporary)
        .witness(Arc::new(move |notice: &Message| {
            app.notices.remember_notice(invocation, notice.into());
        }));

    delivery.notify(&cx.ctx).await;
    cx.trace("notify_target");

    if let Err(failure) = perform(cx, &punishment, &subject).await {
        store::withdraw(cx.pool(), &punishment.id).await?;

        return Err(failure);
    }

    if !keep_offending
        && let Some((channel, message)) = offending
        && let Err(failure) = channel
            .delete_message(&cx.ctx.http, message, None)
            .await
            .ctx("delete the referenced message")
        && !failure.not_found()
    {
        cx.report(&failure);
    }

    let expiry = punishment.expires_at();
    let enqueued = async {
        if let Some(expiry) = expiry
            && let Some(kind) = match punishment.verb {
                PunishmentType::Ban => Some(Kind::LiftBan),
                PunishmentType::Mute => Some(Kind::LiftMute),
                _ => None,
            }
        {
            scheduled::schedule(
                cx.pool(),
                kind,
                &punishment.id,
                punishment.guild,
                punishment.target,
                expiry,
            )
            .await?;
        }

        if punishment.verb != PunishmentType::Mute {
            return Result::Ok(());
        }

        scheduled::schedule(
            cx.pool(),
            Kind::RefreshTimeout,
            &punishment.id,
            punishment.guild,
            punishment.target,
            scheduled::next_refresh(expiry),
        )
        .await
    }
    .await;

    if let Err(failure) = enqueued {
        cx.report(&failure);
    }

    cx.trace("apply_to_discord");

    let evidence = preserve(cx, &punishment, &subject).await;

    cx.trace("preserve_evidence");

    let actor = fetch::user(&cx.ctx, UserId::new(punishment.actor))
        .await
        .ok()
        .map(|found| found.name);

    let entry = ui::log_entry(&punishment, actor.as_deref(), Some(subject.name()))
        .maybe_footnote(evidence.map(|link| format!("[View deleted messages]({link})")));

    let subject_of_log = guildlog::Subject {
        target: punishment.target,
        moderator: Some(punishment.actor),
        action: Some(punishment.id.clone()),
    };

    let controls = records::controls::attached(
        punishment.actor,
        &punishment.id,
        captured.as_ref(),
        punishment.note.as_ref(),
    );

    if let Err(failure) = guildlog::emit(
        cx,
        LogType::MemberModeration,
        &entry,
        subject_of_log,
        &controls,
    )
    .await
    {
        cx.report(&failure);
    }

    cx.trace("write_log_entry");

    if reply == Reply::None {
        return Ok(Response::None);
    }

    let evidence = |mut marks: Marks| {
        marks.has_reference = captured.as_ref().is_some_and(Captured::has_content);
        marks.has_image = captured.as_ref().is_some_and(Captured::has_image);
        marks
    };

    let posted = delivery
        .respond(&cx.ctx, &cx.msg.clone(), |marks| {
            ui::reply(&punishment, evidence(marks))
        })
        .await?;

    cx.app
        .notices
        .remember_reply(invocation, evidence(delivery.marks()));

    Ok(Response::Sent(posted))
}

#[cfg(feature = "web")]
async fn preserve(cx: &Cx, punishment: &Punishment, subject: &Subject) -> Option<String> {
    if punishment.clear_days == 0 {
        return None;
    }

    let since = Utc::now() - Duration::days(punishment.clear_days as i64);

    if let Err(failure) =
        archive::store::removed_since(cx.pool(), punishment.guild, punishment.target, since).await
    {
        cx.report(&failure);
    }

    let name = match subject {
        Subject::Present(member) => member.user.name.to_string(),
        Subject::Absent(user) => user.name.to_string(),
    };

    let asked = transcript::Request::cleared(
        punishment.guild,
        punishment.target,
        name,
        since,
        cx.guild_name().await,
    );

    match transcripts::build(cx.pool(), &asked).await {
        Ok(None) => None,
        Ok(Some(id)) => transcript::url(cx.app.config.web_url.as_deref(), punishment.guild, &id),
        Err(failure) => {
            cx.report(&failure);

            None
        }
    }
}

#[cfg(not(feature = "web"))]
async fn preserve(_cx: &Cx, _punishment: &Punishment, _subject: &Subject) -> Option<String> {
    None
}

async fn perform(cx: &Cx, punishment: &Punishment, subject: &Subject) -> Result<()> {
    let guild = cx.guild_id()?;
    let http = &cx.ctx.http;
    let target = subject.id();
    let audit = punishment.audit_marker();

    if matches!(
        punishment.verb,
        PunishmentType::Mute | PunishmentType::Unmute
    ) {
        cx.app.pending.expect_timeout(guild.get(), target.get());
    }

    match punishment.verb {
        PunishmentType::Warn => Ok(()),
        PunishmentType::Kick => guild
            .kick(http, target, Some(&audit))
            .await
            .ctx("kick member"),
        PunishmentType::Ban => guild
            .ban(http, target, u32::from(punishment.clear_days), Some(&audit))
            .await
            .ctx("ban member"),
        PunishmentType::Softban => {
            guild
                .ban(http, target, u32::from(punishment.clear_days), Some(&audit))
                .await
                .ctx("softban member")?;

            guild.unban(http, target, None).await.ctx("lift softban")
        }
        PunishmentType::Unban => guild.unban(http, target, None).await.ctx("unban user"),
        PunishmentType::Mute => guild
            .edit_member(
                http,
                target,
                EditMember::new()
                    .audit_log_reason(&audit)
                    .disable_communication_until(punishment.timeout_until().into()),
            )
            .await
            .map(|_| ())
            .ctx("mute member"),
        PunishmentType::Unmute => guild
            .edit_member(
                http,
                target,
                EditMember::new()
                    .audit_log_reason(&audit)
                    .enable_communication(),
            )
            .await
            .map(|_| ())
            .ctx("unmute member"),
    }
}
