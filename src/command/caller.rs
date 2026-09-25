use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use serenity::all::{Context, GenericChannelId, GuildId, Member, User, UserId};
use tokio::sync::OnceCell;

use crate::app::App;
use crate::command::error::{Error, Result};
use crate::domain::Snowflake;
use crate::platform::discord::fetch::{self, Overwrites};
use crate::platform::discord::permissions::{Actor, Snapshot};

pub struct Caller {
    pub app: Arc<App>,
    pub ctx: Context,
    guild_id: Option<GuildId>,
    channel_id: GenericChannelId,
    author_id: UserId,
    input: Arc<str>,
    members: Mutex<HashMap<UserId, Member>>,
    guild: OnceCell<Snapshot>,
    actor: OnceCell<Member>,
    bot: OnceCell<Member>,
    overwrites: OnceCell<Overwrites>,
}

impl Caller {
    pub fn new(
        app: Arc<App>,
        ctx: Context,
        guild_id: Option<GuildId>,
        channel_id: GenericChannelId,
        author_id: UserId,
        input: Arc<str>,
    ) -> Self {
        Self {
            app,
            ctx,
            guild_id,
            channel_id,
            author_id,
            input,
            members: Mutex::new(HashMap::new()),
            guild: OnceCell::new(),
            actor: OnceCell::new(),
            bot: OnceCell::new(),
            overwrites: OnceCell::new(),
        }
    }

    pub fn input(&self) -> &str {
        &self.input
    }

    pub fn guild_id(&self) -> Result<GuildId> {
        self.guild_id
            .ok_or_else(|| Error::new(self.input()).title("commands only work in servers"))
    }

    pub fn channel_id(&self) -> GenericChannelId {
        self.channel_id
    }

    pub fn author_id(&self) -> UserId {
        self.author_id
    }

    pub fn bot_id(&self) -> UserId {
        self.ctx.cache.current_user().id
    }

    pub async fn guild(&self) -> Result<&Snapshot> {
        let guild = self.guild_id()?;

        self.guild
            .get_or_try_init(|| fetch::snapshot(&self.ctx, guild))
            .await
    }

    pub async fn actor(&self) -> Result<&Member> {
        let guild = self.guild_id()?;
        let author = self.author_id();

        self.actor
            .get_or_try_init(|| fetch::member(&self.ctx, guild, author))
            .await
    }

    pub async fn bot_member(&self) -> Result<&Member> {
        let guild = self.guild_id()?;
        let bot = self.bot_id();

        self.bot
            .get_or_try_init(|| fetch::member(&self.ctx, guild, bot))
            .await
    }

    pub async fn overwrites(&self) -> Result<&Overwrites> {
        let guild = self.guild_id()?;
        let channel = self.channel_id();

        self.overwrites
            .get_or_try_init(|| fetch::overwrites(&self.ctx, guild, channel))
            .await
    }

    pub async fn member(&self, user: UserId) -> Result<Member> {
        if let Some(known) = self
            .members
            .lock()
            .ok()
            .and_then(|seen| seen.get(&user).cloned())
        {
            return Ok(known);
        }

        let fetched = fetch::member(&self.ctx, self.guild_id()?, user).await?;

        if let Ok(mut seen) = self.members.lock() {
            seen.insert(user, fetched.clone());
        }

        Ok(fetched)
    }

    pub async fn user(&self, user: UserId) -> Result<User> {
        if let Some(known) = self
            .members
            .lock()
            .ok()
            .and_then(|seen| seen.get(&user).cloned())
        {
            return Ok(known.user);
        }

        fetch::user(&self.ctx, user).await
    }

    pub async fn has(&self, wanted: serenity::all::Permissions) -> Result<bool> {
        let guild = self.guild().await?;
        let actor = self.actor().await?;
        let overwrites = self.overwrites().await?;

        Ok(guild.allows(
            Actor {
                id: actor.user.id,
                roles: &actor.roles,
            },
            &overwrites.entries,
            wanted,
        ))
    }

    pub async fn can_target(&self, target: &Member, wanted: serenity::all::Permissions) -> bool {
        let (Ok(guild), Ok(actor)) = (self.guild().await, self.actor().await) else {
            return false;
        };

        guild.can_target(
            Actor {
                id: actor.user.id,
                roles: &actor.roles,
            },
            Actor {
                id: target.user.id,
                roles: &target.roles,
            },
            wanted,
        )
    }

    pub async fn bot_can_target(
        &self,
        target: &Member,
        wanted: serenity::all::Permissions,
    ) -> bool {
        let (Ok(guild), Ok(bot)) = (self.guild().await, self.bot_member().await) else {
            return false;
        };

        guild.can_enforce(
            Actor {
                id: bot.user.id,
                roles: &bot.roles,
            },
            Actor {
                id: target.user.id,
                roles: &target.roles,
            },
            wanted,
        )
    }

    pub fn guild_snowflake(&self) -> Result<Snowflake> {
        Ok(self.guild_id()?.get())
    }

    pub fn pool(&self) -> &sqlx::PgPool {
        &self.app.pool
    }

    pub async fn guild_name(&self) -> String {
        let Ok(guild) = self.guild_id() else {
            return String::from("UNKNOWN_GUILD");
        };

        fetch::guild_name(&self.ctx, guild).await
    }
}
