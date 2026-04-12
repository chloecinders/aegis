use serenity::all::{Context, GuildChannel, Member, PartialGuild, Permissions};
use std::{collections::HashMap, sync::Arc};
use tokio::sync::Mutex;

use crate::{
    commands::Command,
    event_handler::Handler,
    utils::{check_guild_permission, permissions::check_channel_permission},
};

#[derive(Default)]
pub struct PermissionCache {
    inner: HashMap<u64, GuildPermissionCache>,
    user: HashMap<u64, HashMap<String, (bool, CommandPermissionResult)>>,
    user_valid: bool,
}

impl PermissionCache {
    pub fn new() -> Self {
        Self::default()
    }

    pub async fn can_run(&mut self, request: CommandPermissionRequest) -> CommandPermissionResult {
        let cmd_perms = request.command.get_permissions();

        if !self.user_valid {
            self.user = Default::default();
            self.user_valid = true;
        }

        let user_channel_entry = self.user.entry(request.channel.id.get()).or_default();
        let user_cmd_entry = user_channel_entry
            .entry(request.command.get_name().to_string())
            .or_default();

        if !user_cmd_entry.0 {
            for perm in cmd_perms.bot {
                if !check_channel_permission(
                    &request.guild,
                    &request.channel,
                    &request.current_user,
                    perm,
                ) {
                    user_cmd_entry.0 = true;
                    user_cmd_entry.1 = CommandPermissionResult::FailedBot(perm);
                    return user_cmd_entry.1.clone();
                }
            }

            user_cmd_entry.0 = true;
            user_cmd_entry.1 = CommandPermissionResult::Success;
        } else if user_cmd_entry.0 && user_cmd_entry.1 != CommandPermissionResult::Success {
            return user_cmd_entry.1.clone();
        }

        let guild_entry = self.inner.entry(request.guild.id.get()).or_default();
        guild_entry.can_run(request).await
    }

    pub async fn invalidate(&mut self, ctx: &Context, guild_id: u64, user_id: u64) {
        if ctx.cache.current_user().id.get() == user_id {
            self.user_valid = false;
        }

        self.inner
            .entry(guild_id)
            .or_default()
            .invalidate(user_id)
            .await;
    }

    pub async fn invalidate_guild(&mut self, guild_id: u64) {
        self.user_valid = false;
        self.inner.insert(guild_id, Default::default());
    }

    pub async fn byte_footprint(&self) -> usize {
        let mut size = std::mem::size_of::<Self>();
        size += self.inner.capacity()
            * (std::mem::size_of::<u64>() + std::mem::size_of::<GuildPermissionCache>());
        size += self.user.capacity()
            * (std::mem::size_of::<u64>()
                + std::mem::size_of::<HashMap<String, (bool, CommandPermissionResult)>>());

        for map in self.user.values() {
            size += map.capacity()
                * (std::mem::size_of::<String>()
                    + std::mem::size_of::<(bool, CommandPermissionResult)>());
            for k in map.keys() {
                size += k.capacity();
            }
        }

        for guild_cache in self.inner.values() {
            for arc in guild_cache.inner.values() {
                let lock = arc.lock().await;
                size += std::mem::size_of::<tokio::sync::Mutex<CommandPermissionCacheInfo>>();
                size += lock.allowed.capacity()
                    * (std::mem::size_of::<String>()
                        + std::mem::size_of::<CommandPermissionResult>());
                for k in lock.allowed.keys() {
                    size += k.capacity();
                }
            }
        }
        size
    }
}

#[derive(Default)]
struct GuildPermissionCache {
    inner: HashMap<u64, Arc<Mutex<CommandPermissionCacheInfo>>>,
}

impl GuildPermissionCache {
    pub async fn can_run(&mut self, request: CommandPermissionRequest) -> CommandPermissionResult {
        let perms = request.command.get_permissions();

        if perms.one_of.is_empty() && perms.required.is_empty() {
            return CommandPermissionResult::Success;
        }

        let user_id = request.member.user.id.get();
        let command_name = request.command.get_name().to_string();
        let user_entry_arc = Arc::clone(self.inner.entry(user_id).or_default());
        let mut user_entry = user_entry_arc.lock().await;

        if !user_entry.valid {
            let allowed = Self::evaluate_permissions(request.clone()).await;
            user_entry
                .allowed
                .insert(command_name.clone(), allowed.clone());
            user_entry.valid = true;

            let handler = request.handler.clone();
            let member = request.member.clone();
            let guild = request.guild.clone();
            let channel = request.channel.clone();
            let current_user = request.current_user.clone();
            let entry_ref = Arc::clone(&user_entry_arc);

            tokio::spawn(async move {
                let member = member;
                let guild = guild;

                for command in handler.commands.clone() {
                    let perms = command.get_permissions();

                    if perms.one_of.is_empty() && perms.required.is_empty() {
                        continue;
                    }

                    let req = CommandPermissionRequest {
                        current_user: current_user.clone(),
                        command: command.clone(),
                        member: member.clone(),
                        channel: channel.clone(),
                        guild: guild.clone(),
                        handler: handler.clone(),
                    };

                    let ok = Self::evaluate_permissions(req).await;

                    let mut lock = entry_ref.lock().await;
                    lock.allowed.insert(command.get_name().to_string(), ok);
                }
            });

            return allowed;
        }

        match user_entry.allowed.get(&command_name) {
            Some(v) => v.clone(),
            None => {
                let allowed = Self::evaluate_permissions(request).await;
                user_entry.allowed.insert(command_name, allowed.clone());
                allowed
            }
        }
    }

    pub async fn invalidate(&mut self, user_id: u64) {
        let mut entry = self.inner.entry(user_id).or_default().lock().await;
        entry.valid = false;
        entry.allowed = Default::default();
    }

    async fn evaluate_permissions(request: CommandPermissionRequest) -> CommandPermissionResult {
        let permissions = request.command.get_permissions();
        let guild = request.guild;
        let member = request.member;

        if check_guild_permission(&guild, &member, Permissions::ADMINISTRATOR).await
            || guild.owner_id.get() == member.user.id.get()
        {
            return CommandPermissionResult::Success;
        }

        for permission in permissions.required {
            if !check_guild_permission(&guild, &member, permission).await {
                return CommandPermissionResult::FailedUserRequired;
            }
        }

        if !permissions.one_of.is_empty() {
            for permission in permissions.one_of {
                if check_guild_permission(&guild, &member, permission).await {
                    return CommandPermissionResult::Success;
                }
            }

            return CommandPermissionResult::FailedUserOneOf;
        }

        CommandPermissionResult::Success
    }
}

#[derive(Default, Debug, Clone)]
struct CommandPermissionCacheInfo {
    pub allowed: HashMap<String, CommandPermissionResult>,
    pub valid: bool,
}

#[derive(Clone)]
pub struct CommandPermissionRequest {
    pub current_user: Member,
    pub command: Arc<dyn Command>,
    pub member: Member,
    pub guild: PartialGuild,
    pub channel: GuildChannel,
    pub handler: Handler,
}

#[derive(PartialEq, Debug, Clone)]
pub enum CommandPermissionResult {
    Success,
    FailedBot(Permissions),
    FailedUserOneOf,
    FailedUserRequired,
    Uninitialised,
}

impl Default for CommandPermissionResult {
    fn default() -> Self {
        Self::Uninitialised
    }
}
