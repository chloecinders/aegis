use std::collections::HashMap;
use std::sync::Arc;

use serenity::all::{
    ComponentInteraction, ComponentInteractionDataKind, Context, CreateInteractionResponse,
    CreateInteractionResponseMessage, CreateModal, GenericChannelId, GuildId, LabelComponent,
    Member, MessageId, ModalComponent, ModalInteraction, ModalInteractionData, Permissions, User,
};

use crate::app::App;
use crate::command::Boxed;
use crate::command::error::{Ctx, Error, Result};
use crate::domain::Snowflake;
use crate::platform::observe::report::Origin;
use crate::platform::ui::embed::Embed;
use crate::platform::ui::error as render;
use crate::platform::ui::reply::{self, Button};

pub struct Custom {
    key: &'static str,
    owner: Snowflake,
    parts: Vec<String>,
}

impl Custom {
    pub fn new<I, S>(key: &'static str, owner: Snowflake, parts: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        Self {
            key,
            owner,
            parts: parts
                .into_iter()
                .map(|part| part.as_ref().to_string())
                .collect(),
        }
    }

    pub fn render(&self) -> Option<String> {
        if self.parts.iter().any(|part| part.contains(':')) {
            return None;
        }

        let mut id = String::from(self.key);

        id.push(':');
        id.push_str(&self.owner.to_string());

        for part in &self.parts {
            id.push(':');
            id.push_str(part);
        }

        (id.chars().count() <= 100).then_some(id)
    }
}

fn split(custom: &str) -> Option<(&str, Snowflake, Vec<String>)> {
    let mut fields = custom.split(':');
    let key = fields.next().filter(|key| !key.is_empty())?;
    let owner = fields.next()?.parse::<Snowflake>().ok()?;

    Some((key, owner, fields.map(String::from).collect()))
}

pub struct Interacted {
    pub user: User,
    pub permissions: Permissions,
    pub guild: Option<GuildId>,
    pub channel: GenericChannelId,
    pub message: Option<MessageId>,
}

pub struct Click {
    pub app: Arc<App>,
    pub ctx: Context,
    pub interaction: Interacted,
    owner: Snowflake,
    aside: bool,
    parts: Vec<String>,
    supplied: Vec<String>,
}

impl Click {
    pub fn part(&self, at: usize) -> Option<&str> {
        self.parts.get(at).map(String::as_str)
    }

    fn raised_it(&self) -> bool {
        self.owner == self.interaction.user.id.get()
    }

    pub fn owner(&self) -> Snowflake {
        match self.aside {
            true => self.interaction.user.id.get(),
            false => self.owner,
        }
    }

    pub fn guild(&self) -> Option<Snowflake> {
        self.interaction.guild.map(|guild| guild.get())
    }

    pub fn chosen(&self) -> Vec<&str> {
        self.supplied.iter().map(String::as_str).collect()
    }

    fn where_from(&self) -> Origin {
        Origin {
            command: None,
            guild: self.guild(),
            channel: Some(self.interaction.channel.get()),
            user: Some(self.interaction.user.id.get()),
            message: self.interaction.message.map(|message| message.get()),
        }
    }
}

pub struct Rewrite {
    pub embed: Embed,
    pub buttons: Vec<Button>,
}

pub enum Reaction {
    Replace(Box<Rewrite>),
    Aside(Box<Rewrite>),
    Private(Box<Embed>),
    Open(Box<CreateModal<'static>>),
    Dismiss,
    Nothing,
}

impl Reaction {
    pub fn replace(embed: Embed, buttons: Vec<Button>) -> Self {
        Reaction::Replace(Box::new(Rewrite { embed, buttons }))
    }

    pub fn private(embed: Embed) -> Self {
        Reaction::Private(Box::new(embed))
    }

    fn aside(self) -> Self {
        match self {
            Reaction::Replace(rewrite) => Reaction::Aside(rewrite),
            Reaction::Dismiss => Reaction::Nothing,
            other => other,
        }
    }
}

pub fn stale() -> Embed {
    render::render(&Error::bare().title("stale control"))
}

pub type Handler = for<'a> fn(&'a Click) -> Boxed<'a, Result<Reaction>>;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Strangers {
    Fork,
    Deny,
}

pub struct Control {
    pub key: &'static str,
    pub user: Permissions,
    pub one_of: Permissions,
    pub strangers: Strangers,
    pub handle: Handler,
}

#[derive(Default)]
pub struct Router {
    controls: HashMap<&'static str, Control>,
}

impl Router {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add(&mut self, control: Control) {
        self.controls.insert(control.key, control);
    }

    pub fn find(&self, key: &str) -> Option<&Control> {
        self.controls.get(key)
    }

    pub fn keys(&self) -> Vec<&'static str> {
        let mut keys: Vec<&'static str> = self.controls.keys().copied().collect();

        keys.sort_unstable();
        keys
    }
}

fn allowed(click: &Click, control: &Control) -> Result<()> {
    let permissions = click.interaction.permissions;
    let one_of = control.one_of.is_empty() || permissions.intersects(control.one_of);

    if !permissions.contains(control.user) || !one_of {
        return Err(Error::bare().title("missing required permissions"));
    }

    if control.strangers == Strangers::Deny && !click.raised_it() {
        return Err(Error::bare().title("not your interaction"));
    }

    Ok(())
}

async fn reaction(click: &Click, control: &Control) -> Reaction {
    let ran = match allowed(click, control) {
        Ok(()) => (control.handle)(click).await,
        Err(denied) => Err(denied),
    };

    match ran {
        Ok(reaction) => reaction,
        Err(failure) => {
            click.app.reporter.record(&failure, click.where_from());

            Reaction::private(render::render(&failure))
        }
    }
}

fn filled(data: &ModalInteractionData) -> Vec<String> {
    data.components
        .iter()
        .filter_map(|component| match component {
            ModalComponent::Label(label) => match &label.component {
                LabelComponent::InputText(input) => Some(&input.value),
                _ => None,
            },
            _ => None,
        })
        .map(ToString::to_string)
        .collect()
}

fn wielded(member: Option<&Member>) -> Permissions {
    member
        .and_then(|member| member.permissions)
        .unwrap_or_else(Permissions::empty)
}

pub async fn dispatch(app: Arc<App>, ctx: Context, interaction: ComponentInteraction) {
    let custom = interaction.data.custom_id.clone();

    let Some((key, owner, parts)) = split(&custom) else {
        return respond(&ctx, &interaction, Reaction::private(stale())).await;
    };

    let Some(control) = app.controls.find(key) else {
        return respond(&ctx, &interaction, Reaction::private(stale())).await;
    };

    let click = Click {
        app: Arc::clone(&app),
        ctx: ctx.clone(),
        interaction: Interacted {
            user: interaction.user.clone(),
            permissions: wielded(interaction.member.as_deref()),
            guild: interaction.guild_id,
            channel: interaction.channel_id,
            message: Some(interaction.message.id),
        },
        owner,
        aside: control.strangers == Strangers::Fork && owner != interaction.user.id.get(),
        parts,
        supplied: match &interaction.data.kind {
            ComponentInteractionDataKind::StringSelect { values } => values.to_vec(),
            _ => Vec::new(),
        },
    };

    let answered = reaction(&click, control).await;
    let answered = match click.aside {
        true => answered.aside(),
        false => answered,
    };

    respond(&ctx, &interaction, answered).await;
}

pub async fn submitted(app: Arc<App>, ctx: Context, interaction: ModalInteraction) {
    let custom = interaction.data.custom_id.clone();

    let Some((key, owner, parts)) = split(&custom) else {
        return answer_submission(&ctx, &interaction, Reaction::private(stale())).await;
    };

    let Some(control) = app.controls.find(key) else {
        return answer_submission(&ctx, &interaction, Reaction::private(stale())).await;
    };

    let click = Click {
        app: Arc::clone(&app),
        ctx: ctx.clone(),
        interaction: Interacted {
            user: interaction.user.clone(),
            permissions: wielded(interaction.member.as_deref()),
            guild: interaction.guild_id,
            channel: interaction.channel_id,
            message: interaction.message.as_ref().map(|message| message.id),
        },
        owner,
        aside: control.strangers == Strangers::Fork && owner != interaction.user.id.get(),
        parts,
        supplied: filled(&interaction.data),
    };

    let answered = reaction(&click, control).await;
    let answered = match click.aside {
        true => answered.aside(),
        false => answered,
    };

    answer_submission(&ctx, &interaction, answered).await;
}

fn answer(reaction: Reaction) -> CreateInteractionResponse<'static> {
    match reaction {
        Reaction::Replace(rewrite) => CreateInteractionResponse::UpdateMessage(
            CreateInteractionResponseMessage::new()
                .embed(rewrite.embed.build().into_owned())
                .components(
                    rewrite
                        .buttons
                        .chunks(5)
                        .take(5)
                        .map(reply::row)
                        .collect::<Vec<_>>(),
                ),
        ),
        Reaction::Aside(rewrite) => CreateInteractionResponse::Message(
            CreateInteractionResponseMessage::new()
                .embed(rewrite.embed.build().into_owned())
                .components(
                    rewrite
                        .buttons
                        .chunks(5)
                        .take(5)
                        .map(reply::row)
                        .collect::<Vec<_>>(),
                )
                .ephemeral(true),
        ),
        Reaction::Private(embed) => CreateInteractionResponse::Message(
            CreateInteractionResponseMessage::new()
                .embed(embed.build().into_owned())
                .ephemeral(true),
        ),
        Reaction::Open(form) => CreateInteractionResponse::Modal(*form),
        Reaction::Dismiss | Reaction::Nothing => CreateInteractionResponse::Acknowledge,
    }
}

async fn respond(ctx: &Context, interaction: &ComponentInteraction, reaction: Reaction) {
    let dismissed = matches!(reaction, Reaction::Dismiss);
    let sent = interaction
        .create_response(&ctx.http, answer(reaction))
        .await;

    if let Err(failure) = sent.ctx("answer component click") {
        return tracing::warn!("could not answer a control; err = {failure}");
    }

    if dismissed {
        sweep(ctx, interaction).await;
    }
}

async fn sweep(ctx: &Context, interaction: &ComponentInteraction) {
    if let Some(asked) = interaction
        .message
        .message_reference
        .as_ref()
        .and_then(|reference| reference.message_id)
    {
        let _ = interaction
            .channel_id
            .delete_message(&ctx.http, asked, None)
            .await;
    }

    let _ = interaction.message.delete(&ctx.http, None).await;
}

async fn answer_submission(ctx: &Context, interaction: &ModalInteraction, reaction: Reaction) {
    let reaction = match reaction {
        Reaction::Open(_) => Reaction::Nothing,
        other => other,
    };

    let sent = interaction
        .create_response(&ctx.http, answer(reaction))
        .await;

    if let Err(failure) = sent.ctx("answer form submission") {
        tracing::warn!("could not answer a form; err = {failure}");
    }
}
