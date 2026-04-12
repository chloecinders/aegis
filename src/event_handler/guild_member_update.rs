use std::{collections::HashSet, io::Cursor};

use image::{DynamicImage, GenericImage, imageops::FilterType};
use reqwest::Client;
use serenity::all::{
    Context, CreateAttachment, CreateEmbed, CreateEmbedAuthor, CreateMessage,
    GuildMemberUpdateEvent, Member, MemberAction, Mentionable, audit_log::Action,
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

    let mut moderator_id: Option<u64> = None;
    let mut reason: Option<String> = None;
    let old_nick: Option<Option<String>> = old_if_available.clone().map(|o| o.nick);

    if let Some(log) = find_audit_log(
        &ctx,
        event.guild_id,
        Action::Member(MemberAction::Update),
        |a| a.target_id.map(|id| id.get()).unwrap_or(0) == event.user.id.get(),
    )
    .await
    .or(find_audit_log(
        &ctx,
        event.guild_id,
        Action::Member(MemberAction::RoleUpdate),
        |a| a.target_id.map(|id| id.get()).unwrap_or(0) == event.user.id.get(),
    )
    .await)
    {
        moderator_id = Some(log.user_id.get());
        reason = log.reason.clone();
    }

    let name = if let Some(old) = old_nick {
        if old == event.nick {
            String::new()
        } else {
            format!(
                "\n\nName:\n`{}` -> `{}`",
                old.unwrap_or(String::from("(none)")),
                event.nick.unwrap_or(String::from("(none)"))
            )
        }
    } else {
        String::new()
    };

    if let Some(old) = old_if_available.clone()
        && let Some(new) = new.clone()
    {
        let lhs = old.avatar.or(old.user.avatar);
        let rhs = new.avatar.or(new.user.avatar);

        if matches!(
            (lhs, rhs, old.user.avatar, new.user.avatar),
            (Some(a), Some(b), Some(ua_old), Some(ua_new))
                if a == ua_old && b == ua_new
        ) {
            let client = Client::new();

            if let (Some(old_image), Some(new_image)) = (
                get_member_avatar_image(&client, old).await,
                get_member_avatar_image(&client, new).await,
            ) {
                let target_height = old_image.height();

                let old_image = old_image.resize(
                    old_image.width() * target_height / old_image.height(),
                    target_height,
                    FilterType::Lanczos3,
                );

                let new_image = new_image.resize(
                    new_image.width() * target_height / new_image.height(),
                    target_height,
                    FilterType::Lanczos3,
                );

                let total_width = target_height * 2;
                let mut output = DynamicImage::new_rgba8(total_width, target_height);

                output.copy_from(&old_image, 0, 0).unwrap();
                output.copy_from(&new_image, new_image.width(), 0).unwrap();

                let mut buff = Vec::new();
                if output
                    .write_to(&mut Cursor::new(&mut buff), image::ImageFormat::WebP)
                    .is_ok()
                {
                    let description = format!("**AVATAR UPDATE**\n-# <@{}>", event.user.id);

                    let embed = CreateEmbed::new()
                        .color(BRAND_BLUE)
                        .description(description)
                        .author(
                            CreateEmbedAuthor::new(format!(
                                "{}: {}",
                                event.user.name,
                                event.user.id.get()
                            ))
                            .icon_url(
                                event
                                    .user
                                    .avatar_url()
                                    .unwrap_or(event.user.default_avatar_url()),
                            ),
                        )
                        .image("attachment://avatar.webp");

                    let msg = CreateMessage::new()
                        .add_embed(embed)
                        .add_file(CreateAttachment::bytes(buff, "avatar.webp"));

                    guild_log(&ctx, LogType::AvatarUpdate, event.guild_id, msg, None).await;
                    return;
                }
            };
        }
    };

    let roles = if let Some(old) = old_if_available
        && let Some(new) = new
    {
        let old_set: HashSet<_> = old.roles.iter().cloned().map(|r| (r, ())).collect();
        let new_set: HashSet<_> = new.roles.iter().cloned().map(|r| (r, ())).collect();

        let removed = old_set
            .difference(&new_set)
            .cloned()
            .map(|r| r.0.mention().to_string())
            .collect::<Vec<_>>()
            .join(" ");

        let added = new_set
            .difference(&old_set)
            .cloned()
            .map(|r| r.0.mention().to_string())
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
    };

    if (name.is_empty() && roles.is_empty())
        || (name.is_empty()
            && !roles.is_empty()
            && moderator_id.unwrap_or(0) == event.user.id.get())
    {
        return;
    }

    let moderator = if let Some(id) = moderator_id {
        format!(" | Actor: <@{id}>")
    } else {
        String::new()
    };

    let reason = if let Some(reason) = reason {
        format!("\nReason:\n```{reason} ```")
    } else {
        String::new()
    };

    let description = format!(
        "**MEMBER UPDATE**\n-# <@{}>{}{}{}{}",
        event.user.id, moderator, name, reason, roles
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

async fn get_member_avatar_image(client: &Client, member: Member) -> Option<image::DynamicImage> {
    let avatar_req = client
        .get(
            member.avatar_url().unwrap_or(
                member
                    .user
                    .avatar_url()
                    .unwrap_or(member.user.default_avatar_url()),
            ),
        )
        .send()
        .await
        .ok()?;
    let bytes = avatar_req.bytes().await.ok()?;
    image::load_from_memory(&bytes).ok()
}
