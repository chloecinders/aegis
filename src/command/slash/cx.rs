use std::ops::Deref;
use std::sync::Arc;

use serenity::all::{CommandInteraction, Context, ResolvedValue, Unresolved};

use crate::app::App;
use crate::command::caller::Caller;
use crate::command::error::Error;
use crate::platform::observe::report::Origin;

pub struct SlashCx {
    caller: Caller,
    pub interaction: CommandInteraction,
    command: &'static str,
}

impl Deref for SlashCx {
    type Target = Caller;

    fn deref(&self) -> &Caller {
        &self.caller
    }
}

fn option_text(value: &ResolvedValue<'_>) -> Option<String> {
    Some(match value {
        ResolvedValue::Boolean(flag) => flag.to_string(),
        ResolvedValue::Integer(number) => number.to_string(),
        ResolvedValue::Number(number) => number.to_string(),
        ResolvedValue::String(text) => text.to_string(),
        ResolvedValue::Autocomplete { value, .. } => value.to_string(),
        ResolvedValue::Attachment(attachment) => attachment.filename.to_string(),
        ResolvedValue::Channel(channel) => format!("<#{}>", channel.id()),
        ResolvedValue::Role(role) => format!("<@&{}>", role.id),
        ResolvedValue::User(user, _) => format!("<@{}>", user.id),
        ResolvedValue::Unresolved(Unresolved::Channel(id)) => format!("<#{id}>"),
        ResolvedValue::Unresolved(Unresolved::RoleId(id)) => format!("<@&{id}>"),
        ResolvedValue::Unresolved(Unresolved::User(id)) => format!("<@{id}>"),
        _ => return None,
    })
}

fn input(interaction: &CommandInteraction) -> Arc<str> {
    let mut line = format!("/{}", interaction.data.name);

    for option in interaction.data.options() {
        if let Some(value) = option_text(&option.value) {
            line.push_str(&format!(" {}:{value}", option.name));
        }
    }

    Arc::from(line)
}

impl SlashCx {
    pub fn new(
        app: Arc<App>,
        ctx: Context,
        interaction: CommandInteraction,
        command: &'static str,
    ) -> Self {
        Self {
            caller: Caller::new(
                app,
                ctx,
                interaction.guild_id,
                interaction.channel_id,
                interaction.user.id,
                input(&interaction),
            ),
            interaction,
            command,
        }
    }

    pub fn origin(&self) -> Origin {
        Origin {
            command: Some(self.command),
            guild: self.interaction.guild_id.map(|guild| guild.get()),
            channel: Some(self.channel_id().get()),
            user: Some(self.author_id().get()),
            message: None,
        }
    }

    pub fn report(&self, failure: &Error) {
        self.app.reporter.record(failure, self.origin());
    }
}
