use std::cmp::Reverse;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use serde::Serialize;
use serenity::all::{ChannelType, GenericChannelId, GuildId, Permissions, UserId};
use serenity::http::Http;

use crate::domain::Snowflake;
use crate::platform::discord::fetch;
use crate::platform::discord::permissions::{Actor, Snapshot};

#[derive(Clone, Debug, Serialize)]
pub struct Entry {
    #[serde(with = "crate::web::flat")]
    pub id: Snowflake,
    pub name: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct View {
    #[serde(with = "crate::web::flat")]
    pub id: Snowflake,
    pub name: String,
    pub icon: Option<String>,
    pub roles: Vec<Entry>,
    pub channels: Vec<Entry>,
}

pub trait Directory: Send + Sync {
    fn present(&self, guild: Snowflake) -> bool;

    fn view(&self, guild: Snowflake) -> Pin<Box<dyn Future<Output = Option<View>> + Send + '_>>;

    fn readable(
        &self,
        guild: Snowflake,
        viewer: Snowflake,
        channels: Vec<Snowflake>,
    ) -> Pin<Box<dyn Future<Output = Vec<Snowflake>> + Send + '_>>;
}

pub struct Present {
    pub http: Arc<Http>,
    pub cache: Arc<serenity::cache::Cache>,
}

impl Present {
    async fn sees(
        &self,
        guild: GuildId,
        snapshot: &Snapshot,
        actor: Actor<'_>,
        channel: GenericChannelId,
    ) -> bool {
        let discord = (Some(&self.cache), &*self.http);
        let administers = snapshot.base(actor).contains(Permissions::ADMINISTRATOR);

        let Ok(found) = fetch::overwrites(discord, guild, channel).await else {
            return administers;
        };

        let granted = snapshot.in_channel(actor, &found.entries);

        if !granted.contains(Permissions::VIEW_CHANNEL)
            || !granted.contains(Permissions::READ_MESSAGE_HISTORY)
        {
            return false;
        }

        if found.kind != ChannelType::PrivateThread || granted.contains(Permissions::MANAGE_THREADS)
        {
            return true;
        }

        self.http
            .get_channel_thread_members(channel.expect_thread())
            .await
            .is_ok_and(|joined| joined.iter().any(|member| member.user_id == actor.id))
    }
}

impl Directory for Present {
    fn present(&self, guild: Snowflake) -> bool {
        self.cache.guild(GuildId::new(guild)).is_some()
    }

    fn view(&self, guild: Snowflake) -> Pin<Box<dyn Future<Output = Option<View>> + Send + '_>> {
        Box::pin(async move {
            let id = GuildId::new(guild);
            let found = self.http.get_guild(id).await.ok()?;
            let channels = self.http.get_channels(id).await.unwrap_or_default();

            let mut roles: Vec<&serenity::all::Role> = found
                .roles
                .iter()
                .filter(|role| role.id.get() != guild)
                .collect();

            roles.sort_by_key(|left| Reverse(left.position));

            let mut listed: Vec<&serenity::all::GuildChannel> = channels
                .iter()
                .filter(|channel| writable(channel))
                .collect();

            listed.sort_by_key(|channel| channel.position);

            Some(View {
                id: guild,
                name: found.name.to_string(),
                icon: found
                    .icon
                    .map(|hash| crate::web::oauth::guild_icon(guild, &hash.to_string())),
                roles: roles
                    .into_iter()
                    .map(|role| Entry {
                        id: role.id.get(),
                        name: role.name.to_string(),
                    })
                    .collect(),
                channels: listed
                    .into_iter()
                    .map(|channel| Entry {
                        id: channel.id.get(),
                        name: channel.base.name.to_string(),
                    })
                    .collect(),
            })
        })
    }

    fn readable(
        &self,
        guild: Snowflake,
        viewer: Snowflake,
        channels: Vec<Snowflake>,
    ) -> Pin<Box<dyn Future<Output = Vec<Snowflake>> + Send + '_>> {
        Box::pin(async move {
            let id = GuildId::new(guild);
            let discord = (Some(&self.cache), &*self.http);

            let Ok(snapshot) = fetch::snapshot(discord, id).await else {
                return Vec::new();
            };

            let Ok(member) = fetch::member(discord, id, UserId::new(viewer)).await else {
                return Vec::new();
            };

            let actor = Actor {
                id: member.user.id,
                roles: &member.roles,
            };

            let mut allowed = Vec::new();

            for channel in channels {
                if self
                    .sees(id, &snapshot, actor, GenericChannelId::new(channel))
                    .await
                {
                    allowed.push(channel);
                }
            }

            allowed
        })
    }
}

pub fn writable(channel: &serenity::all::GuildChannel) -> bool {
    matches!(
        channel.base.kind,
        ChannelType::Text
            | ChannelType::News
            | ChannelType::Forum
            | ChannelType::Voice
            | ChannelType::Stage
            | ChannelType::PublicThread
            | ChannelType::PrivateThread
            | ChannelType::NewsThread
    )
}
