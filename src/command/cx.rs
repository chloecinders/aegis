use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use serenity::all::{
    Context, CreateComponent, EditMessage, GenericChannelId, GuildId, Member, Message, MessageId,
    User, UserId,
};
use tokio::sync::OnceCell;

use crate::app::App;
use crate::command::error::{Ctx as _, Error, Result};
use crate::domain::Snowflake;
use crate::domain::ids::ActionId;
use crate::platform::discord::fetch::{self, Overwrites};
use crate::platform::discord::permissions::{Actor, Snapshot};
use crate::platform::observe::report::Origin;
use crate::platform::observe::trace::Trace;
use crate::platform::ui::embed::Embed;
use crate::platform::ui::reply;

#[derive(Clone, Debug)]
pub struct Invocation {
    pub command: &'static str,
    pub args: serde_json::Value,
}

pub struct Cx {
    pub app: Arc<App>,
    pub ctx: Context,
    pub msg: Arc<Message>,
    input: Arc<str>,
    revising: Option<MessageId>,
    trace: Mutex<Trace>,
    invocation: Mutex<Option<Invocation>>,
    action: Mutex<Option<ActionId>>,
    members: Mutex<HashMap<UserId, Member>>,
    guild: OnceCell<Snapshot>,
    actor: OnceCell<Member>,
    bot: OnceCell<Member>,
    overwrites: OnceCell<Overwrites>,
}

impl Cx {
    pub fn new(app: Arc<App>, ctx: Context, msg: Arc<Message>) -> Self {
        let input = Arc::from(msg.content.as_str());

        Self::reading(app, ctx, msg, input)
    }

    pub fn reading(app: Arc<App>, ctx: Context, msg: Arc<Message>, input: Arc<str>) -> Self {
        Self {
            app,
            ctx,
            msg,
            input,
            revising: None,
            trace: Mutex::new(Trace::new()),
            invocation: Mutex::new(None),
            action: Mutex::new(None),
            members: Mutex::new(HashMap::new()),
            guild: OnceCell::new(),
            actor: OnceCell::new(),
            bot: OnceCell::new(),
            overwrites: OnceCell::new(),
        }
    }

    pub fn amending(mut self, response: Option<MessageId>) -> Self {
        self.revising = response;
        self
    }

    pub fn revision(&self) -> Option<MessageId> {
        self.revising
    }

    pub fn input(&self) -> &str {
        &self.input
    }

    pub fn guild_id(&self) -> Result<GuildId> {
        self.msg
            .guild_id
            .ok_or_else(|| Error::new(self.input()).title("command only works in servers"))
    }

    pub fn channel_id(&self) -> GenericChannelId {
        self.msg.channel_id
    }

    pub fn author_id(&self) -> UserId {
        self.msg.author.id
    }

    pub fn bot_id(&self) -> UserId {
        self.ctx.cache.current_user().id
    }

    pub async fn present(
        &self,
        embed: &Embed,
        rows: Vec<CreateComponent<'static>>,
        op: &'static str,
    ) -> Result<MessageId> {
        let Some(response) = self.revising else {
            let sent = self
                .channel_id()
                .send_message(
                    &self.ctx.http,
                    reply::plain(embed)
                        .components(rows)
                        .reference_message(&*self.msg),
                )
                .await
                .ctx(op)?;

            return Ok(sent.id);
        };

        self.channel_id()
            .edit_message(
                &self.ctx.http,
                response,
                EditMessage::new()
                    .embeds(vec![embed.build().into_owned()])
                    .components(rows),
            )
            .await
            .ctx(op)?;

        Ok(response)
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

    pub fn trace(&self, name: &'static str) {
        if let Ok(mut trace) = self.trace.lock() {
            trace.point(name);
        }
    }

    pub fn trace_snapshot(&self) -> Trace {
        self.trace
            .lock()
            .map(|trace| trace.clone())
            .unwrap_or_default()
    }

    pub fn remember(&self, command: &'static str, args: serde_json::Value) {
        if let Ok(mut slot) = self.invocation.lock() {
            *slot = Some(Invocation { command, args });
        }
    }

    pub fn note_action(&self, id: ActionId) {
        if let Ok(mut slot) = self.action.lock() {
            *slot = Some(id);
        }
    }

    pub fn action(&self) -> Option<ActionId> {
        self.action.lock().ok().and_then(|slot| slot.clone())
    }

    pub fn invocation(&self) -> Option<Invocation> {
        self.invocation.lock().ok().and_then(|slot| slot.clone())
    }

    pub fn origin(&self) -> Origin {
        Origin {
            command: self.invocation().map(|record| record.command),
            guild: self.msg.guild_id.map(|guild| guild.get()),
            channel: Some(self.channel_id().get()),
            user: Some(self.author_id().get()),
            message: Some(self.msg.id.get()),
        }
    }

    pub fn report(&self, failure: &Error) {
        self.app.reporter.record(failure, self.origin());
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
