use serenity::all::{
    ButtonStyle, CreateActionRow, CreateAllowedMentions, CreateButton, CreateComponent,
    CreateMessage, CreateSelectMenu, CreateSelectMenuKind, CreateSelectMenuOption,
};

use crate::platform::text::truncate;
use crate::platform::ui::embed::Embed;

pub fn plain(embed: &Embed) -> CreateMessage<'static> {
    CreateMessage::new()
        .add_embed(embed.build().into_owned())
        .allowed_mentions(CreateAllowedMentions::new().replied_user(false))
}

pub fn row(buttons: &[Button]) -> CreateComponent<'static> {
    CreateComponent::ActionRow(CreateActionRow::Buttons(
        buttons
            .iter()
            .map(|button| {
                CreateButton::new(button.id.clone())
                    .label(button.label.clone())
                    .style(button.style)
                    .disabled(button.disabled)
            })
            .collect(),
    ))
}

#[derive(Clone, Debug)]
pub struct Button {
    pub id: String,
    pub label: String,
    pub style: ButtonStyle,
    pub disabled: bool,
}

impl Button {
    pub fn new(id: String, label: impl Into<String>, style: ButtonStyle) -> Self {
        Self {
            id,
            label: label.into(),
            style,
            disabled: false,
        }
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
}

#[derive(Clone, Debug)]
pub struct Choice {
    pub value: String,
    pub label: String,
    pub description: String,
}

impl Choice {
    pub fn new(
        value: impl Into<String>,
        label: impl Into<String>,
        description: impl Into<String>,
    ) -> Self {
        Self {
            value: value.into(),
            label: label.into(),
            description: description.into(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct Menu {
    pub id: String,
    pub placeholder: String,
    pub choices: Vec<Choice>,
}

impl Menu {
    pub fn new(id: String, placeholder: impl Into<String>, choices: Vec<Choice>) -> Self {
        Self {
            id,
            placeholder: placeholder.into(),
            choices,
        }
    }

    fn build(&self) -> CreateComponent<'static> {
        let options: Vec<CreateSelectMenuOption> = self
            .choices
            .iter()
            .take(25)
            .map(|choice| {
                CreateSelectMenuOption::new(
                    truncate::clamp(&choice.label, 100),
                    choice.value.clone(),
                )
                .description(truncate::clamp(&choice.description, 100))
            })
            .collect();
        let picked = options.len().max(1) as u8;

        CreateComponent::ActionRow(CreateActionRow::SelectMenu(
            CreateSelectMenu::new(
                self.id.clone(),
                CreateSelectMenuKind::String {
                    options: options.into(),
                },
            )
            .placeholder(self.placeholder.clone())
            .min_values(1)
            .max_values(picked),
        ))
    }
}

#[derive(Clone, Debug, Default)]
pub struct Panel {
    pub menu: Option<Menu>,
    pub buttons: Vec<Button>,
}

impl Panel {
    pub fn new(menu: Menu, buttons: Vec<Button>) -> Self {
        Self {
            menu: Some(menu),
            buttons,
        }
    }

    pub fn rows(&self) -> Vec<CreateComponent<'static>> {
        let Some(menu) = &self.menu else {
            return self.buttons.chunks(5).take(5).map(row).collect();
        };

        let mut out = vec![menu.build()];

        out.extend(self.buttons.chunks(5).take(4).map(row));
        out
    }
}

impl From<Vec<Button>> for Panel {
    fn from(buttons: Vec<Button>) -> Self {
        Self {
            menu: None,
            buttons,
        }
    }
}
