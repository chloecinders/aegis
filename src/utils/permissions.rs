use serenity::all::{
    Context, GuildChannel, Member, PartialGuild, PermissionOverwriteType, Permissions, User,
};

use crate::BOT_CONFIG;

/// Checks if a member has a permission in the guild. Ingnores channel overrides.
pub async fn check_guild_permission(
    guild: &PartialGuild,
    member: &Member,
    permission: Permissions,
) -> bool {
    if guild.owner_id.get() == member.user.id.get() {
        return true;
    }

    for role in member.roles.clone() {
        let Some(role) = guild.roles.get(&role) else {
            return false;
        };

        if role.has_permission(permission) || role.has_permission(Permissions::ADMINISTRATOR) {
            return true;
        }
    }

    false
}

/// Checks if a member has a permission in a guilds channel. Respects channel overrides.
pub fn check_channel_permission(
    guild: &PartialGuild,
    channel: &GuildChannel,
    member: &Member,
    permission: Permissions,
) -> bool {
    if guild.owner_id.get() == member.user.id.get() {
        return true;
    }

    let channel_perms = permissions_for_channel(guild, channel, member);
    channel_perms.contains(Permissions::ADMINISTRATOR) || channel_perms.contains(permission)
}

/// Gets all the permissions of a member in a guild.
pub fn permissions_for_guild(guild: &PartialGuild, member: &Member) -> Permissions {
    let everyone = guild.roles.iter().find(|r| r.1.position == 0).unwrap();
    let mut roles = member
        .roles
        .iter()
        .map(|r| guild.roles.get(r).unwrap())
        .collect::<Vec<_>>();
    roles.push(everyone.1);
    roles.sort();

    let mut base = Permissions::empty();

    for role in roles {
        role.permissions.into_iter().for_each(|p| {
            base.insert(p);
        });
    }

    base
}

/// Gets all the permissions of a member in a guild channel, including channel overrides.
pub fn permissions_for_channel(
    guild: &PartialGuild,
    channel: &GuildChannel,
    member: &Member,
) -> Permissions {
    let mut permissions = permissions_for_guild(guild, member);
    let everyone = guild.roles.iter().find(|r| r.1.position == 0).unwrap();
    let mut roles = member
        .roles
        .iter()
        .map(|r| guild.roles.get(r).unwrap())
        .collect::<Vec<_>>();
    roles.push(everyone.1);
    roles.sort();

    let overwrites = channel.permission_overwrites.clone();

    for role in roles {
        if let Some(overwrite) = overwrites.iter().find(|p| {
            if let PermissionOverwriteType::Role(id) = p.kind
                && id == role.id
            {
                true
            } else {
                false
            }
        }) {
            for perm in overwrite.allow {
                permissions.insert(perm);
            }

            for perm in overwrite.deny {
                permissions.remove(perm);
            }
        }
    }

    if let Some(overwrite) = overwrites.iter().find(|p| {
        let PermissionOverwriteType::Member(id) = p.kind else {
            return false;
        };
        id == member.user.id
    }) {
        for perm in overwrite.allow {
            permissions.insert(perm);
        }

        for perm in overwrite.deny {
            permissions.remove(perm);
        }
    }

    permissions
}

/// Checks if a user is a developer using the BOT_CONFIG.
pub fn is_developer(user: &User) -> bool {
    BOT_CONFIG
        .dev_ids
        .clone()
        .is_some_and(|i| i.contains(&user.id.get()))
}

/// Checks if a user can target another user with a specific permission (i.e. can user ban target?)
pub async fn can_target(
    ctx: &Context,
    user: &Member,
    target: &Member,
    permission: Permissions,
) -> bool {
    if let Ok(partial) = user.guild_id.to_partial_guild(ctx).await {
        if user.user.id == partial.owner_id {
            return true;
        };
        if target.user.id == partial.owner_id {
            return false;
        };
    }

    let get_highest_role_pos = async |mem: &Member| {
        let mut matching = -1;

        // fetch roles if they dont exist in the cache
        let mut roles = {
            if let Some(roles) = mem.roles(&ctx) {
                roles
            } else {
                if let Ok(roles) = mem.guild_id.roles(&ctx).await {
                    mem.roles
                        .iter()
                        .filter_map(|r| roles.get(r).cloned())
                        .collect()
                } else {
                    vec![]
                }
            }
        };

        roles.sort();

        for role in roles {
            if role.has_permission(permission) || role.has_permission(Permissions::ADMINISTRATOR) {
                matching = role.position as i32;
            }
        }

        matching
    };

    let user_highest_matching_role_pos = get_highest_role_pos(user).await;
    let target_highest_matching_role_pos = get_highest_role_pos(target).await;
    user_highest_matching_role_pos > target_highest_matching_role_pos
}
