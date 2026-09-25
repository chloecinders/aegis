use std::ops::Deref;
use std::sync::{Arc, Mutex};

use serenity::all::{Context, CreateComponent, EditMessage, Message, MessageId};

use crate::app::App;
use crate::command::caller::Caller;
use crate::command::error::{Ctx as _, Error, Result};
use crate::domain::ids::ActionId;
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
    caller: Caller,
    pub msg: Arc<Message>,
    revising: Option<MessageId>,
    trace: Mutex<Trace>,
    invocation: Mutex<Option<Invocation>>,
    action: Mutex<Option<ActionId>>,
}

impl Deref for Cx {
    type Target = Caller;

    fn deref(&self) -> &Caller {
        &self.caller
    }
}

impl Cx {
    pub fn new(app: Arc<App>, ctx: Context, msg: Arc<Message>) -> Self {
        let input = Arc::from(msg.content.as_str());

        Self::reading(app, ctx, msg, input)
    }

    pub fn reading(app: Arc<App>, ctx: Context, msg: Arc<Message>, input: Arc<str>) -> Self {
        Self {
            caller: Caller::new(app, ctx, msg.guild_id, msg.channel_id, msg.author.id, input),
            msg,
            revising: None,
            trace: Mutex::new(Trace::new()),
            invocation: Mutex::new(None),
            action: Mutex::new(None),
        }
    }

    pub fn amending(mut self, response: Option<MessageId>) -> Self {
        self.revising = response;
        self
    }

    pub fn revision(&self) -> Option<MessageId> {
        self.revising
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
}
