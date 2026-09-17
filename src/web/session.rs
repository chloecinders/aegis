use std::collections::HashMap;
use std::sync::RwLock;

use chrono::{DateTime, Duration, Utc};
use rand::RngCore;
use serde::{Deserialize, Serialize};
use serenity::all::Permissions;
use sha2::{Digest, Sha256};
use sqlx::PgPool;
use tracing::warn;

use crate::domain::Snowflake;

#[derive(Clone, Debug, Serialize)]
pub struct Membership {
    #[serde(with = "crate::web::flat")]
    pub id: Snowflake,
    pub name: String,
    pub icon: Option<String>,
    #[serde(skip)]
    pub permissions: u64,
}

impl Membership {
    pub fn moderates(&self) -> bool {
        Permissions::from_bits_truncate(self.permissions).intersects(
            Permissions::ADMINISTRATOR
                | Permissions::MANAGE_GUILD
                | Permissions::BAN_MEMBERS
                | Permissions::KICK_MEMBERS
                | Permissions::MANAGE_MESSAGES
                | Permissions::MODERATE_MEMBERS,
        )
    }

    pub fn administers(&self) -> bool {
        Permissions::from_bits_truncate(self.permissions)
            .intersects(Permissions::ADMINISTRATOR | Permissions::MANAGE_GUILD)
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct Session {
    #[serde(with = "crate::web::flat")]
    pub user: Snowflake,
    pub name: String,
    pub display: Option<String>,
    pub avatar: Option<String>,
    pub guilds: Vec<Membership>,
    pub expires: DateTime<Utc>,
}

#[derive(Clone, Debug)]
struct Handoff {
    destination: String,
    expires: DateTime<Utc>,
}

#[derive(Serialize, Deserialize)]
struct StoredMembership {
    id: String,
    name: String,
    icon: Option<String>,
    permissions: String,
}

impl StoredMembership {
    fn of(membership: &Membership) -> Self {
        Self {
            id: membership.id.to_string(),
            name: membership.name.clone(),
            icon: membership.icon.clone(),
            permissions: membership.permissions.to_string(),
        }
    }

    fn back(self) -> Option<Membership> {
        Some(Membership {
            id: self.id.parse().ok()?,
            name: self.name,
            icon: self.icon,
            permissions: self.permissions.parse().ok()?,
        })
    }
}

pub struct Sessions {
    pool: PgPool,
    open: RwLock<HashMap<String, Session>>,
    handoffs: RwLock<HashMap<String, Handoff>>,
}

impl Sessions {
    pub fn new(pool: PgPool) -> Self {
        Self {
            pool,
            open: RwLock::default(),
            handoffs: RwLock::default(),
        }
    }

    pub async fn open(&self, session: Session) -> String {
        let token = secret();

        if let Ok(mut sessions) = self.open.write() {
            sessions.retain(|_, open| open.expires > Utc::now());
            sessions.insert(token.clone(), session.clone());
        }

        self.keep(&token, &session).await;

        token
    }

    async fn keep(&self, token: &str, session: &Session) {
        let guilds = session
            .guilds
            .iter()
            .map(StoredMembership::of)
            .collect::<Vec<_>>();

        let Ok(guilds) = serde_json::to_value(guilds) else {
            return warn!("a session's guilds would not serialise; it will not survive a restart");
        };

        let written = sqlx::query!(
            "INSERT INTO dashboard_sessions (token, account_id, name, display, avatar, guilds, expires)
            VALUES ($1, $2, $3, $4, $5, $6, $7)",
            crate::platform::text::hex(&Sha256::digest(token.as_bytes())),
            session.user as i64,
            session.name,
            session.display.as_deref(),
            session.avatar.as_deref(),
            guilds,
            session.expires
        )
        .execute(&self.pool)
        .await;

        if let Err(failure) = written {
            warn!("could not store the session; it dies on restart; err = {failure}");

            return;
        }

        let swept = sqlx::query!("DELETE FROM dashboard_sessions WHERE expires <= now()")
            .execute(&self.pool)
            .await;

        if let Err(failure) = swept {
            warn!("could not sweep expired sessions ({failure})");
        }
    }

    pub async fn read(&self, token: &str) -> Option<Session> {
        let found = match self
            .open
            .read()
            .ok()
            .and_then(|sessions| sessions.get(token).cloned())
        {
            Some(found) => found,
            None => {
                let found = self.stored(token).await?;

                if let Ok(mut sessions) = self.open.write() {
                    sessions.insert(token.to_string(), found.clone());
                }

                found
            }
        };

        if found.expires <= Utc::now() {
            self.close(token).await;

            return None;
        }

        Some(found)
    }

    async fn stored(&self, token: &str) -> Option<Session> {
        let row = sqlx::query!(
            "SELECT account_id, name, display, avatar, guilds, expires
            FROM dashboard_sessions WHERE token = $1",
            crate::platform::text::hex(&Sha256::digest(token.as_bytes()))
        )
        .fetch_optional(&self.pool)
        .await;

        let row = match row {
            Ok(row) => row?,
            Err(failure) => {
                warn!("could not read a session ({failure})");

                return None;
            }
        };

        let Ok(guilds) = serde_json::from_value::<Vec<StoredMembership>>(row.guilds) else {
            warn!("a stored session's guilds would not parse; treating it as signed out");

            return None;
        };

        Some(Session {
            user: row.account_id as Snowflake,
            name: row.name,
            display: row.display,
            avatar: row.avatar,
            guilds: guilds
                .into_iter()
                .filter_map(StoredMembership::back)
                .collect(),
            expires: row.expires,
        })
    }

    pub async fn close(&self, token: &str) {
        if let Ok(mut sessions) = self.open.write() {
            sessions.remove(token);
        }

        let deleted = sqlx::query!(
            "DELETE FROM dashboard_sessions WHERE token = $1",
            crate::platform::text::hex(&Sha256::digest(token.as_bytes()))
        )
        .execute(&self.pool)
        .await;

        if let Err(failure) = deleted {
            warn!("could not close a stored session; it expires on its own; err = {failure}");
        }
    }

    pub fn begin(&self, destination: &str) -> String {
        let state = secret();

        if let Ok(mut handoffs) = self.handoffs.write() {
            handoffs.retain(|_, pending| pending.expires > Utc::now());
            handoffs.insert(
                state.clone(),
                Handoff {
                    destination: destination.to_string(),
                    expires: Utc::now() + Duration::minutes(10),
                },
            );
        }

        state
    }

    pub fn finish(&self, state: &str) -> Option<String> {
        let pending = self.handoffs.write().ok()?.remove(state)?;

        match pending.expires > Utc::now() {
            true => Some(pending.destination),
            false => None,
        }
    }
}

fn secret() -> String {
    let mut bytes = [0u8; 32];

    rand::rng().fill_bytes(&mut bytes);

    use base64::Engine;

    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes)
}

pub fn from_cookies(header: Option<&str>) -> Option<&str> {
    header?.split(';').find_map(|pair| {
        let (name, value) = pair.split_once('=')?;

        match name.trim() == "aegis_session" {
            true => Some(value.trim()),
            false => None,
        }
    })
}

pub fn handed(token: &str, site: &str) -> String {
    let secure = match site.starts_with("https://") {
        true => "; Secure",
        false => "",
    };

    format!(
        "aegis_session={token}; Path=/; HttpOnly; SameSite=Lax; Max-Age={}{secure}",
        Duration::hours(12).num_seconds()
    )
}

pub fn cleared(site: &str) -> String {
    let secure = match site.starts_with("https://") {
        true => "; Secure",
        false => "",
    };

    format!("aegis_session=; Path=/; HttpOnly; SameSite=Lax; Max-Age=0{secure}")
}
