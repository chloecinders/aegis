use std::collections::HashMap;

use serenity::all::{
    CacheHttp, ChannelId, ChannelType, GenericChannelId, GenericGuildChannelRef, GuildChannel,
    GuildId, Member, PermissionOverwrite, Permissions, RoleId, User, UserId,
};
use serenity::small_fixed_array::FixedArray;

use crate::command::error::{Ctx, Error, Result};
use crate::platform::discord::permissions::{Role, Snapshot};

pub struct Overwrites {
    pub kind: ChannelType,
    pub entries: FixedArray<PermissionOverwrite>,
}

pub async fn snapshot(discord: impl CacheHttp, guild: GuildId) -> Result<Snapshot> {
    if let Some(cached) = discord.cache().and_then(|cache| cache.guild(guild)) {
        return Ok(Snapshot {
            guild,
            owner: cached.owner_id,
            roles: roles(
                cached
                    .roles
                    .iter()
                    .map(|role| (role.id, role.permissions, role.position)),
            ),
        });
    }

    let fetched = guild
        .to_partial_guild(discord.http())
        .await
        .ctx("fetch guild")?;

    Ok(Snapshot {
        guild,
        owner: fetched.owner_id,
        roles: roles(
            fetched
                .roles
                .iter()
                .map(|role| (role.id, role.permissions, role.position)),
        ),
    })
}

fn roles(roles: impl Iterator<Item = (RoleId, Permissions, i16)>) -> HashMap<RoleId, Role> {
    roles
        .map(|(id, permissions, position)| {
            (
                id,
                Role {
                    permissions,
                    position: position as i64,
                },
            )
        })
        .collect()
}

pub async fn member(discord: impl CacheHttp, guild: GuildId, user: UserId) -> Result<Member> {
    if let Some(cached) = discord
        .cache()
        .and_then(|cache| cache.guild(guild))
        .and_then(|guild| guild.members.get(&user).cloned())
    {
        return Ok(cached);
    }

    guild.member(discord.http(), user).await.ctx("fetch member")
}

pub async fn channel(
    discord: impl CacheHttp,
    guild: GuildId,
    channel: ChannelId,
) -> Result<GuildChannel> {
    let fetched = channel
        .to_guild_channel(&discord, Some(guild))
        .await
        .ctx("fetch channel")?;

    match fetched.base.guild_id == guild {
        true => Ok(fetched),
        false => Err(Error::empty().title("channel not in this server")),
    }
}

pub async fn overwrites(
    discord: impl CacheHttp,
    guild: GuildId,
    channel: GenericChannelId,
) -> Result<Overwrites> {
    let cached = discord.cache().and_then(|cache| {
        let cached = cache.guild(guild)?;

        match cached.channel(channel)? {
            GenericGuildChannelRef::Channel(found) => Some(Overwrites {
                kind: found.base.kind,
                entries: found.permission_overwrites.clone(),
            }),
            GenericGuildChannelRef::Thread(thread) => {
                let parent = cached.channels.get(&thread.parent_id)?;

                Some(Overwrites {
                    kind: thread.base.kind,
                    entries: parent.permission_overwrites.clone(),
                })
            }
        }
    });

    if let Some(found) = cached {
        return Ok(found);
    }

    let (channel_id, thread_id) = channel.split();

    if let Ok(found) = channel_id.to_guild_channel(&discord, Some(guild)).await {
        return Ok(Overwrites {
            kind: found.base.kind,
            entries: found.permission_overwrites,
        });
    }

    let thread = thread_id
        .to_thread(&discord, Some(guild))
        .await
        .ctx("fetch thread")?;

    let parent = self::channel(&discord, guild, thread.parent_id).await?;

    Ok(Overwrites {
        kind: thread.base.kind,
        entries: parent.permission_overwrites,
    })
}

pub async fn user(discord: impl CacheHttp, user: UserId) -> Result<User> {
    discord.http().get_user(user).await.ctx("fetch user")
}

pub async fn guild_name(discord: impl CacheHttp, guild: GuildId) -> String {
    if let Some(cached) = discord.cache().and_then(|cache| cache.guild(guild)) {
        return cached.name.to_string();
    }

    guild
        .to_partial_guild(discord.http())
        .await
        .ctx("fetch guild name")
        .map(|fetched| fetched.name.to_string())
        .unwrap_or_else(|_| String::from("UNKNOWN_GUILD"))
}
