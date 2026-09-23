use std::sync::Arc;

use serenity::all::audit_log::{Action, Change, ChannelAction, MemberAction, MessageAction};
use serenity::all::{AuditLogEntry, ChannelId, Context, GuildId};

use crate::app::App;
use crate::domain::punishment::PunishmentType;
use crate::features::archive;
use crate::features::guildlog;
use crate::features::guildlog::attribution::Attribution;
use crate::features::guildlog::member::Part;
use crate::features::punishments::external;
use crate::platform::discord::fetch;

pub async fn record(app: &Arc<App>, ctx: &Context, entry: AuditLogEntry, guild: GuildId) {
    let bot = ctx.cache.current_user().id.get();

    let Some(actor) = entry.user_id else {
        return;
    };

    match entry.action {
        Action::Message(MessageAction::Delete) => {
            let (Some(target), Some(channel)) = (
                entry.target_id,
                entry
                    .options
                    .as_ref()
                    .and_then(|options| options.channel_id),
            ) else {
                return;
            };

            guildlog::amend::attribute_deletion(
                app,
                ctx,
                (guild.get(), target.get(), channel.get()),
                actor.get(),
                bot,
            )
            .await;
        }
        Action::Message(MessageAction::BulkDelete) => {
            if actor.get() == bot {
                return;
            }

            let Some(target) = entry.target_id else {
                return;
            };

            guildlog::amend::attribute_bulk(
                app,
                ctx,
                (guild.get(), target.get()),
                actor.get(),
                bot,
            )
            .await;
        }
        Action::Channel(ChannelAction::Delete) => {
            let Some(target) = entry.target_id else {
                return;
            };

            let name = ctx.cache.guild(guild).and_then(|cached| {
                cached
                    .channels
                    .get(&ChannelId::new(target.get()))
                    .map(|channel| channel.base.name.to_string())
            });

            archive::deletion::channel(app, ctx, guild, target.get(), name, actor.get(), bot).await;
        }
        Action::Member(MemberAction::Update | MemberAction::RoleUpdate) => {
            let Some(target) = entry.target_id else {
                return;
            };

            let guild = guild.get();
            let target = target.get();
            let actor = actor.get();
            let changes = &entry.changes;
            let parts = parts(changes);
            let reason = entry
                .reason
                .as_deref()
                .map(str::trim)
                .filter(|given| !given.is_empty());

            guildlog::amend::attribute_update(app, ctx, guild, target, &parts, actor, reason, bot)
                .await;

            let (before, after) = changes
                .iter()
                .find_map(|change| match change {
                    Change::CommunicationDisabledUntil { old, new } => {
                        Some((old.map(|at| *at), new.map(|at| *at)))
                    }
                    _ => None,
                })
                .unwrap_or((None, None));

            let recorded = external::observe(
                app,
                ctx,
                external::Involved {
                    guild,
                    actor,
                    target,
                    bot,
                },
                before,
                after,
                reason,
            )
            .await;

            if let Err(failure) = recorded {
                app.reporter.record(&failure, Default::default());
            }
        }
        Action::Member(what @ (MemberAction::Kick | MemberAction::BanAdd)) => {
            let Some(target) = entry.target_id else {
                return;
            };

            let verb = match what {
                MemberAction::Kick => PunishmentType::Kick,
                _ => PunishmentType::Ban,
            };

            let recorded = external::removed(
                app,
                ctx,
                external::Involved {
                    guild: guild.get(),
                    actor: actor.get(),
                    target: target.get(),
                    bot,
                },
                verb,
                entry.reason.as_deref(),
            )
            .await;

            if let Err(failure) = recorded {
                app.reporter.record(&failure, Default::default());
            }
        }
        other => {
            let actor_name = fetch::user(ctx, actor).await.ok().map(|found| found.name);
            let seen = guildlog::audit::Event {
                action: other,
                target: entry.target_id.map(|target| target.get()),
                actor: Attribution::Gateway(actor.get()),
                actor_name: actor_name.as_deref(),
                bot,
                changes: &entry.changes,
                status: entry
                    .options
                    .as_ref()
                    .and_then(|options| options.status.as_deref()),
                reason: entry.reason.as_deref(),
            };

            let Some(logged) = guildlog::audit::render(&seen) else {
                return;
            };

            let outcome = guildlog::post(
                app,
                ctx,
                guild.get(),
                logged.kind,
                &logged.embed,
                guildlog::Subject {
                    target: seen.target.unwrap_or_default(),
                    moderator: Some(actor.get()),
                    action: None,
                },
            )
            .await;

            if let Err(failure) = outcome {
                app.reporter.record(&failure, Default::default());
            }
        }
    }
}

fn parts(changes: &[Change]) -> Vec<Part> {
    let mut parts = Vec::new();

    for change in changes {
        match change {
            Change::Nick { .. } => parts.push(Part::Nick),
            Change::CommunicationDisabledUntil { .. } => parts.push(Part::Timeout),
            Change::RolesAdded { old, new } => parts.extend(
                old.iter()
                    .chain(new)
                    .flatten()
                    .map(|role| Part::Gained(role.id.get())),
            ),
            Change::RolesRemoved { old, new } => parts.extend(
                old.iter()
                    .chain(new)
                    .flatten()
                    .map(|role| Part::Lost(role.id.get())),
            ),
            _ => (),
        }
    }

    parts
}
