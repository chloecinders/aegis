use serenity::all::{
    Context, CreateEmbed, CreateEmbedAuthor, CreateMessage, GuildId, GuildMemberUpdateEvent,
    Member, MemberAction, Mentionable,
    audit_log::{Action, Change},
};

use crate::{
    constants::BRAND_BLUE,
    event_handler::Handler,
    utils::{LogType, find_audit_log, guild_log},
};

pub async fn guild_member_update(
    handler: &Handler,
    ctx: Context,
    old_if_available: Option<Member>,
    new: Option<Member>,
    event: GuildMemberUpdateEvent,
) {
    {
        let mut permission_lock = handler.permission_cache.lock().await;
        permission_lock
            .invalidate(&ctx, event.guild_id.get(), event.user.id.get())
            .await;
    }

    if event.user.bot {
        return;
    }

    let (moderator_id, reason, role_changes_from_log) =
        fetch_audit_log_info(&ctx, event.guild_id, event.user.id.get()).await;

    let old_nick = old_if_available.clone().map(|o| o.nick);
    let name = format_name_change(old_nick, event.nick.clone());

    let old_timeout = old_if_available
        .clone()
        .and_then(|o| o.communication_disabled_until);
    let new_timeout = event.communication_disabled_until;

    handle_timeout_change(
        &ctx,
        &event,
        old_timeout,
        new_timeout,
        moderator_id,
        &reason,
    )
    .await;

    let roles = format_role_change(role_changes_from_log, &old_if_available, &new);

    if name.is_empty() && roles.is_empty() {
        return;
    }

    if name.is_empty() && !roles.is_empty() && moderator_id.unwrap_or(0) == event.user.id.get() {
        return;
    }

    let moderator = if let Some(id) = moderator_id {
        format!(" | Actor: <@{id}>")
    } else {
        String::new()
    };

    let reason_str = if let Some(reason) = reason {
        format!("\nReason:\n```{reason} ```")
    } else {
        String::new()
    };

    let details = format!("{}{}{}", name, reason_str, roles);
    let description = format!(
        "**MEMBER UPDATE**\n-# <@{}>{}\n{}",
        event.user.id,
        moderator,
        details.trim_start()
    );
    let embed = CreateEmbed::new()
        .color(BRAND_BLUE)
        .description(description)
        .author(
            CreateEmbedAuthor::new(format!("{}: {}", event.user.name, event.user.id.get()))
                .icon_url(
                    event
                        .user
                        .avatar_url()
                        .unwrap_or(event.user.default_avatar_url()),
                ),
        );
    let msg = CreateMessage::new().add_embed(embed);

    guild_log(&ctx, LogType::MemberUpdate, event.guild_id, msg, None).await;
}

async fn fetch_audit_log_info(
    ctx: &Context,
    guild_id: GuildId,
    user_id: u64,
) -> (Option<u64>, Option<String>, Option<(String, String)>) {
    if let Some(log) = find_audit_log(ctx, guild_id, Action::Member(MemberAction::Update), |a| {
        a.target_id.map(|id| id.get()).unwrap_or(0) == user_id
    })
    .await
    .or(find_audit_log(
        ctx,
        guild_id,
        Action::Member(MemberAction::RoleUpdate),
        |a| a.target_id.map(|id| id.get()).unwrap_or(0) == user_id,
    )
    .await)
    {
        let moderator_id = Some(log.user_id.get());
        let reason = log.reason.clone();
        let mut role_changes_from_log = None;

        if let Some(changes) = &log.changes {
            let added = changes
                .iter()
                .filter_map(|c| {
                    if let Change::RolesAdded { new, .. } = c {
                        new.as_ref()
                    } else {
                        None
                    }
                })
                .flatten()
                .map(|r| r.id.mention().to_string())
                .collect::<Vec<_>>()
                .join(" ");

            let removed = changes
                .iter()
                .filter_map(|c| {
                    if let Change::RolesRemove { new, .. } = c {
                        new.as_ref()
                    } else {
                        None
                    }
                })
                .flatten()
                .map(|r| r.id.mention().to_string())
                .collect::<Vec<_>>()
                .join(" ");

            if !added.is_empty() || !removed.is_empty() {
                role_changes_from_log = Some((added, removed));
            }
        }

        (moderator_id, reason, role_changes_from_log)
    } else {
        (None, None, None)
    }
}

fn format_name_change(old_nick: Option<Option<String>>, new_nick: Option<String>) -> String {
    if let Some(old) = old_nick {
        if old == new_nick {
            String::new()
        } else {
            format!(
                "\n\nName:\n`{}` -> `{}`",
                old.unwrap_or(String::from("(none)")),
                new_nick.unwrap_or(String::from("(none)"))
            )
        }
    } else {
        String::new()
    }
}

async fn handle_timeout_change(
    ctx: &Context,
    event: &GuildMemberUpdateEvent,
    old_timeout: Option<serenity::all::Timestamp>,
    new_timeout: Option<serenity::all::Timestamp>,
    moderator_id: Option<u64>,
    reason: &Option<String>,
) {
    if old_timeout == new_timeout {
        return;
    }

    if let Some(actor_id) = moderator_id {
        if actor_id == ctx.cache.current_user().id.get() {
            return;
        }

        let (action_title, duration_str) = if let Some(until) = new_timeout {
            let now = serenity::all::Timestamp::now().unix_timestamp();
            let until_ts = until.unix_timestamp();
            if until_ts > now {
                let duration = until_ts - now;
                let time_string = if duration >= 86400 * 27 {
                    String::from("28 days")
                } else if duration >= 86400 {
                    format!("{} days", (duration as f64 / 86400.0).round())
                } else if duration >= 3600 {
                    format!("{} hours", (duration as f64 / 3600.0).round())
                } else if duration >= 60 {
                    format!("{} minutes", (duration as f64 / 60.0).round())
                } else {
                    format!("{} seconds", duration)
                };
                ("MEMBER MUTED", time_string)
            } else {
                ("MEMBER UNMUTED", String::new())
            }
        } else if old_timeout.is_some() {
            ("MEMBER UNMUTED", String::new())
        } else {
            ("", String::new())
        };

        if !action_title.is_empty() {
            let mut description = format!(
                "**{}**\n-# Actor: <@{}> | Target: <@{}>",
                action_title, actor_id, event.user.id
            );

            if !duration_str.is_empty() {
                description.push_str(&format!(" | Duration: {}", duration_str));
            }

            if let Some(reason_text) = reason {
                description.push_str(&format!("\n```\n{}\n```", reason_text));
            }

            guild_log(
                ctx,
                LogType::MemberModeration,
                event.guild_id,
                CreateMessage::new().add_embed(
                    CreateEmbed::new()
                        .description(description)
                        .color(BRAND_BLUE),
                ),
                Some(crate::utils::logging::LogContext {
                    target_id: event.user.id.get(),
                    moderator_id: actor_id,
                    db_id: None,
                    content: None,
                }),
            )
            .await;
        }
    }
}

fn format_role_change(
    role_changes_from_log: Option<(String, String)>,
    old_if_available: &Option<Member>,
    new: &Option<Member>,
) -> String {
    if let Some((added, removed)) = role_changes_from_log {
        format!(
            "\n\nRoles:\n{}{}{}",
            if added.is_empty() {
                String::new()
            } else {
                format!("+{added}")
            },
            if !added.is_empty() && !removed.is_empty() {
                "\n"
            } else {
                ""
            },
            if removed.is_empty() {
                String::new()
            } else {
                format!("-{removed}")
            }
        )
    } else if let (Some(old), Some(new)) = (old_if_available, new) {
        use std::collections::HashSet;
        let old_set: HashSet<_> = old.roles.iter().collect();
        let new_set: HashSet<_> = new.roles.iter().collect();

        let removed = old_set
            .difference(&new_set)
            .map(|r| r.mention().to_string())
            .collect::<Vec<_>>()
            .join(" ");

        let added = new_set
            .difference(&old_set)
            .map(|r| r.mention().to_string())
            .collect::<Vec<_>>()
            .join(" ");

        if !added.is_empty() || !removed.is_empty() {
            format!(
                "\n\nRoles:\n{}{}{}",
                if added.is_empty() {
                    String::new()
                } else {
                    format!("+{added}")
                },
                if !added.is_empty() && !removed.is_empty() {
                    "\n"
                } else {
                    ""
                },
                if removed.is_empty() {
                    String::new()
                } else {
                    format!("-{removed}")
                }
            )
        } else {
            String::new()
        }
    } else {
        String::new()
    }
}
