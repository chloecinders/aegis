pub mod cx;
pub mod run;
pub mod value;

use std::future::Future;

use serenity::all::{CommandOptionType, CommandType, CreateCommand, CreateCommandOption};

use crate::command::Meta;
use crate::command::error::Result;
use crate::command::slash::cx::SlashCx;
use crate::platform::ui::embed::Embed;
use crate::platform::ui::reply::Button;

#[derive(Clone, Copy, Debug)]
pub struct Parameter {
    pub name: &'static str,
    pub kind: CommandOptionType,
    pub desc: &'static str,
    pub required: bool,
}

pub trait Options: Sized + Send + 'static {
    const PARAMETERS: &'static [Parameter];

    fn parse(cx: &SlashCx) -> Result<Self>;
}

pub trait Slash: Options {
    const META: Meta;

    fn run(self, cx: &mut SlashCx) -> impl Future<Output = Result<Reply>> + Send;
}

#[derive(Debug)]
pub struct Reply {
    pub embed: Embed,
    pub buttons: Vec<Button>,
    pub ephemeral: bool,
}

impl Reply {
    pub fn new(embed: Embed) -> Self {
        Self {
            embed,
            buttons: Vec::new(),
            ephemeral: false,
        }
    }

    pub fn buttons(mut self, buttons: Vec<Button>) -> Self {
        self.buttons = buttons;
        self
    }

    pub fn ephemeral(mut self) -> Self {
        self.ephemeral = true;
        self
    }
}

pub fn definition(meta: &Meta, parameters: &[Parameter]) -> CreateCommand<'static> {
    CreateCommand::new(meta.name)
        .kind(CommandType::ChatInput)
        .description(meta.short)
        .dm_permission(false)
        .set_options(
            parameters
                .iter()
                .map(|parameter| {
                    CreateCommandOption::new(parameter.kind, parameter.name, parameter.desc)
                        .required(parameter.required)
                })
                .collect::<Vec<_>>(),
        )
}
