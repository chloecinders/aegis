use serenity::all::{
    ChannelId, Context, GuildId, Member, PermissionOverwrite, PermissionOverwriteType, Permissions,
    RoleId, User, UserId,
};

use crate::BOT_CONFIG;
use std::collections::HashMap;

/// Checks if a member has a permission in the guild. Ignores channel overrides.
pub fn check_guild_permission(
    guild_id: GuildId,
    owner_id: UserId,
    roles: &HashMap<RoleId, serenity::all::Role>,
    member: &Member,
    permission: Permissions,
) -> bool {
    if owner_id == member.user.id {
        return true;
    }

    let permissions = permissions_for_guild(guild_id, owner_id, roles, member);
    permissions.contains(Permissions::ADMINISTRATOR) || permissions.contains(permission)
}

/// Checks if a member has a permission in a guilds channel. Respects channel overrides.
pub fn check_channel_permission(
    guild_id: GuildId,
    owner_id: UserId,
    roles: &HashMap<RoleId, serenity::all::Role>,
    channel_id: ChannelId,
    overwrites: &[PermissionOverwrite],
    member: &Member,
    permission: Permissions,
) -> bool {
    if owner_id == member.user.id {
        return true;
    }

    let channel_perms =
        permissions_for_channel(guild_id, owner_id, roles, channel_id, overwrites, member);
    channel_perms.contains(Permissions::ADMINISTRATOR) || channel_perms.contains(permission)
}

/// Gets all the permissions of a member in a guild.
pub fn permissions_for_guild(
    guild_id: GuildId,
    _owner_id: UserId,
    roles: &HashMap<RoleId, serenity::all::Role>,
    member: &Member,
) -> Permissions {
    let mut base = roles
        .get(&RoleId::new(guild_id.get()))
        .map(|r| r.permissions)
        .unwrap_or_else(Permissions::empty);

    for role_id in &member.roles {
        if let Some(role) = roles.get(role_id) {
            base |= role.permissions;
        }
    }

    base
}

/// Gets all the permissions of a member in a guild channel, including channel overrides.
pub fn permissions_for_channel(
    guild_id: GuildId,
    owner_id: UserId,
    roles: &HashMap<RoleId, serenity::all::Role>,
    _channel_id: ChannelId,
    overwrites: &[PermissionOverwrite],
    member: &Member,
) -> Permissions {
    if owner_id == member.user.id {
        return Permissions::all();
    }

    let mut permissions = permissions_for_guild(guild_id, owner_id, roles, member);

    if permissions.contains(Permissions::ADMINISTRATOR) {
        return Permissions::all();
    }

    if let Some(everyone_overwrite) = overwrites
        .iter()
        .find(|o| o.kind == PermissionOverwriteType::Role(RoleId::new(guild_id.get())))
    {
        permissions = (permissions & !everyone_overwrite.deny) | everyone_overwrite.allow;
    }

    let (mut role_allow, mut role_deny) = (Permissions::empty(), Permissions::empty());
    for role_id in &member.roles {
        if let Some(role_overwrite) = overwrites
            .iter()
            .find(|o| o.kind == PermissionOverwriteType::Role(*role_id))
        {
            role_allow |= role_overwrite.allow;
            role_deny |= role_overwrite.deny;
        }
    }
    permissions = (permissions & !role_deny) | role_allow;

    if let Some(member_overwrite) = overwrites
        .iter()
        .find(|o| o.kind == PermissionOverwriteType::Member(member.user.id))
    {
        permissions = (permissions & !member_overwrite.deny) | member_overwrite.allow;
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
    let owner_id = if let Some(g) = ctx.cache.guild(user.guild_id) {
        Some(g.owner_id)
    } else if let Ok(partial) = user.guild_id.to_partial_guild(ctx).await {
        Some(partial.owner_id)
    } else {
        None
    };

    if let Some(owner_id) = owner_id {
        if user.user.id == owner_id {
            return true;
        };
        if target.user.id == owner_id {
            return false;
        };
    }

    let get_highest_role_pos = async |mem: &Member| {
        let mut matching = -1;

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
