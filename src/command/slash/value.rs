use serenity::all::{
    CommandOptionType, GenericChannelId, ResolvedValue, RoleId, Unresolved, UserId,
};

use crate::command::error::{Error, Result};
use crate::command::slash::cx::SlashCx;

pub trait FromOption: Sized {
    const KIND: CommandOptionType;

    fn from_option(value: &ResolvedValue<'_>) -> Option<Self>;
}

impl FromOption for String {
    const KIND: CommandOptionType = CommandOptionType::String;

    fn from_option(value: &ResolvedValue<'_>) -> Option<Self> {
        match value {
            ResolvedValue::String(text) => Some(text.to_string()),
            _ => None,
        }
    }
}

impl FromOption for i64 {
    const KIND: CommandOptionType = CommandOptionType::Integer;

    fn from_option(value: &ResolvedValue<'_>) -> Option<Self> {
        match value {
            ResolvedValue::Integer(number) => Some(*number),
            _ => None,
        }
    }
}

impl FromOption for bool {
    const KIND: CommandOptionType = CommandOptionType::Boolean;

    fn from_option(value: &ResolvedValue<'_>) -> Option<Self> {
        match value {
            ResolvedValue::Boolean(flag) => Some(*flag),
            _ => None,
        }
    }
}

impl FromOption for UserId {
    const KIND: CommandOptionType = CommandOptionType::User;

    fn from_option(value: &ResolvedValue<'_>) -> Option<Self> {
        match value {
            ResolvedValue::User(user, _) => Some(user.id),
            ResolvedValue::Unresolved(Unresolved::User(id)) => Some(*id),
            _ => None,
        }
    }
}

impl FromOption for GenericChannelId {
    const KIND: CommandOptionType = CommandOptionType::Channel;

    fn from_option(value: &ResolvedValue<'_>) -> Option<Self> {
        match value {
            ResolvedValue::Channel(channel) => Some(channel.id()),
            ResolvedValue::Unresolved(Unresolved::Channel(id)) => Some(*id),
            _ => None,
        }
    }
}

impl FromOption for RoleId {
    const KIND: CommandOptionType = CommandOptionType::Role;

    fn from_option(value: &ResolvedValue<'_>) -> Option<Self> {
        match value {
            ResolvedValue::Role(role) => Some(role.id),
            ResolvedValue::Unresolved(Unresolved::RoleId(id)) => Some(*id),
            _ => None,
        }
    }
}

pub fn optional<T: FromOption>(cx: &SlashCx, name: &str) -> Result<Option<T>> {
    let options = cx.interaction.data.options();

    let Some(given) = options.iter().find(|option| option.name == name) else {
        return Ok(None);
    };

    T::from_option(&given.value)
        .map(Some)
        .ok_or_else(|| Error::internal("slash option arrived with the wrong type"))
}

pub fn required<T: FromOption>(cx: &SlashCx, name: &str) -> Result<T> {
    optional(cx, name)?.ok_or_else(|| Error::internal("required slash option arrived empty"))
}
