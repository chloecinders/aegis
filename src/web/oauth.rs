use reqwest::{Client, Url};
use serde::Deserialize;

use crate::domain::Snowflake;
use crate::web::session::{Membership, Session};

#[derive(Clone, Debug)]
pub struct Oauth {
    pub client_id: String,
    pub client_secret: String,
    pub redirect: String,
}

#[derive(Debug, thiserror::Error)]
pub enum Failure {
    #[error("could not reach Discord: {0}")]
    Unreachable(#[from] reqwest::Error),
    #[error("Discord refused the sign-in")]
    Refused,
    #[error("Discord's answer did not parse")]
    Malformed,
}

pub type Result<T> = std::result::Result<T, Failure>;

impl Oauth {
    pub fn authorize(&self, state: &str) -> String {
        Url::parse_with_params(
            "https://discord.com/oauth2/authorize",
            &[
                ("client_id", self.client_id.as_str()),
                ("redirect_uri", self.redirect.as_str()),
                ("response_type", "code"),
                ("scope", "identify guilds"),
                ("state", state),
                ("prompt", "none"),
            ],
        )
        .map(|built| built.to_string())
        .unwrap_or_else(|_| String::from("https://discord.com/oauth2/authorize"))
    }

    async fn access(&self, client: &Client, code: &str, redirected: bool) -> Result<String> {
        let mut form = vec![
            ("client_id", self.client_id.as_str()),
            ("client_secret", self.client_secret.as_str()),
            ("grant_type", "authorization_code"),
            ("code", code),
        ];

        if redirected {
            form.push(("redirect_uri", self.redirect.as_str()));
        }

        let answer = client
            .post("https://discord.com/api/v10/oauth2/token")
            .form(&form)
            .send()
            .await?;

        if !answer.status().is_success() {
            return Err(Failure::Refused);
        }

        #[derive(Deserialize)]
        struct Granted {
            access_token: String,
        }

        answer
            .json::<Granted>()
            .await
            .map(|granted| granted.access_token)
            .map_err(|_| Failure::Malformed)
    }

    pub async fn identify(
        &self,
        client: &Client,
        code: &str,
        redirected: bool,
    ) -> Result<(Profile, Vec<Membership>)> {
        let token = self.access(client, code, redirected).await?;

        Ok((
            fetch::<Profile>(client, &token, "https://discord.com/api/v10/users/@me").await?,
            fetch::<Vec<Guild>>(
                client,
                &token,
                "https://discord.com/api/v10/users/@me/guilds",
            )
            .await?
            .into_iter()
            .filter_map(Guild::into_membership)
            .collect(),
        ))
    }
}

async fn fetch<T: serde::de::DeserializeOwned>(
    client: &Client,
    token: &str,
    url: &str,
) -> Result<T> {
    let answer = client.get(url).bearer_auth(token).send().await?;

    if !answer.status().is_success() {
        return Err(Failure::Refused);
    }

    answer.json::<T>().await.map_err(|_| Failure::Malformed)
}

#[derive(Debug, Deserialize)]
pub struct Profile {
    pub id: String,
    pub username: String,
    pub global_name: Option<String>,
    pub avatar: Option<String>,
}

impl Profile {
    pub fn user(&self) -> Option<Snowflake> {
        self.id.parse().ok()
    }
}

#[derive(Debug, Deserialize)]
pub struct Guild {
    pub id: String,
    pub name: String,
    pub icon: Option<String>,
    pub permissions: String,
}

impl Guild {
    fn into_membership(self) -> Option<Membership> {
        let id: Snowflake = self.id.parse().ok()?;

        Some(Membership {
            icon: self.icon.map(|hash| guild_icon(id, &hash)),
            id,
            name: self.name,
            permissions: self.permissions.parse().ok()?,
        })
    }
}

pub fn guild_icon(guild: Snowflake, hash: &str) -> String {
    format!("https://cdn.discordapp.com/icons/{guild}/{hash}.png?size=128")
}

pub fn avatar(user: Snowflake, hash: &str) -> String {
    format!("https://cdn.discordapp.com/avatars/{user}/{hash}.png?size=64")
}

pub fn opened(
    profile: Profile,
    guilds: Vec<Membership>,
    expires: chrono::DateTime<chrono::Utc>,
) -> Option<Session> {
    let user = profile.user()?;

    Some(Session {
        user,
        avatar: profile.avatar.as_deref().map(|hash| avatar(user, hash)),
        name: profile.username,
        display: profile.global_name,
        guilds,
        expires,
    })
}
