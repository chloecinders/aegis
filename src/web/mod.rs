pub mod assets;
pub mod dash;
pub mod directory;
pub mod entrypoint;
pub mod middleware;
pub mod oauth;
pub mod routes;
pub mod session;
pub mod site;

use std::future::Future;
use std::net::{Ipv4Addr, SocketAddr};
use std::pin::Pin;
use std::sync::Arc;

use axum::Router;
use axum::routing::{get, post, put};
use serenity::http::Http;
use sqlx::PgPool;
use tokio::net::TcpListener;
use tracing::{info, warn};

use crate::command::error::Result;
use crate::domain::Snowflake;
use crate::features::archive::secrets::Keys;
use crate::features::automod::cache::Rules;
use crate::features::permissions::cache::Permits;
use crate::features::settings::cache::Settings;
use crate::platform::crypto::Secret;
use crate::web::directory::Directory;
use crate::web::oauth::Oauth;
use crate::web::session::Sessions;

pub mod flat {
    use serde::Serializer;

    use crate::domain::Snowflake;

    pub fn serialize<S: Serializer>(value: &Snowflake, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&value.to_string())
    }

    pub mod maybe {
        use serde::Serializer;

        use crate::domain::Snowflake;

        pub fn serialize<S: Serializer>(
            value: &Option<Snowflake>,
            serializer: S,
        ) -> Result<S::Ok, S::Error> {
            match value {
                Some(id) => serializer.serialize_str(&id.to_string()),
                None => serializer.serialize_none(),
            }
        }
    }
}

pub trait Keyring: Send + Sync {
    fn key(
        &self,
        guild: Snowflake,
    ) -> Pin<Box<dyn Future<Output = Result<Option<Secret>>> + Send + '_>>;
}

pub struct Custody {
    pub pool: PgPool,
    pub secrets: Arc<Keys>,
    pub http: Arc<Http>,
}

impl Keyring for Custody {
    fn key(
        &self,
        guild: Snowflake,
    ) -> Pin<Box<dyn Future<Output = Result<Option<Secret>>> + Send + '_>> {
        Box::pin(async move { self.secrets.of(&self.pool, &self.http, guild).await })
    }
}

pub struct Web {
    pub pool: PgPool,
    pub keys: Arc<dyn Keyring>,
    pub guilds: Arc<dyn Directory>,
    pub rules: Arc<Rules>,
    pub settings: Arc<Settings>,
    pub permits: Arc<Permits>,
    pub sessions: Sessions,
    pub client: reqwest::Client,
    pub oauth: Option<Oauth>,
    pub site: String,
    pub developers: Vec<Snowflake>,
}

pub type Shared = Arc<Web>;

pub const SIGN_IN: &str = "/login";
pub const CALLBACK: &str = "/auth/callback";

pub fn router(state: Shared) -> Router {
    Router::new()
        .route("/", get(site::root))
        .route("/transcript/:guild/:id", get(routes::page))
        .route("/api/transcript/:guild/:id", get(routes::meta))
        .route("/api/transcript/:guild/:id/messages", get(routes::messages))
        .route("/health", get(routes::health))
        .route(SIGN_IN, get(dash::auth::sign_in))
        .route(CALLBACK, get(dash::auth::callback))
        .route("/logout", get(dash::auth::sign_out))
        .route("/dashboard", get(assets::dashboard))
        .route("/dashboard/:guild", get(assets::dashboard))
        .route("/dashboard/:guild/automod", get(assets::dashboard))
        .route("/dashboard/managed_rules", get(assets::dashboard))
        .route("/dashboard/:guild/logs", get(assets::dashboard))
        .route("/dashboard/:guild/permissions", get(assets::dashboard))
        .route("/dashboard/:guild/errors", get(assets::dashboard))
        .route("/api/dash/identity", get(dash::auth::identity))
        .route("/api/dash/guilds/:guild", get(dash::auth::guild))
        .route(
            "/api/dash/guilds/:guild/rules",
            get(dash::rules::all).post(dash::rules::create),
        )
        .route(
            "/api/dash/guilds/:guild/rules/:rule",
            put(dash::rules::amend).delete(dash::rules::delete),
        )
        .route(
            "/api/dash/guilds/:guild/managed_rules",
            get(dash::managed::offers),
        )
        .route(
            "/api/dash/guilds/:guild/managed_rules/:rule",
            put(dash::managed::subscribe).delete(dash::managed::unsubscribe),
        )
        .route(
            "/api/dash/managed_rules",
            get(dash::authoring::all).post(dash::authoring::compose),
        )
        .route(
            "/api/dash/managed_rules/:rule",
            put(dash::authoring::revise).delete(dash::authoring::delete),
        )
        .route(
            "/api/dash/guilds/:guild/logs",
            get(dash::logging::logs).put(dash::logging::route),
        )
        .route("/api/dash/guilds/:guild/errors", get(dash::logging::errors))
        .route(
            "/api/dash/guilds/:guild/permissions",
            get(dash::permissions::all).post(dash::permissions::grant),
        )
        .route(
            "/api/dash/guilds/:guild/permissions/:rule",
            put(dash::permissions::retune).delete(dash::permissions::revoke),
        )
        .route("/api/dash/commands", get(dash::catalog::commands))
        .route("/api/dash/check", post(dash::preview::check))
        .route("/api/dash/activity", post(dash::auth::activity))
        .route("/fonts/:name", get(assets::font))
        .route("/activity.js", get(assets::runtime))
        .route("/assets/:app/:file", get(assets::chunk))
        .with_state(state)
        .fallback(site::page)
        .layer(axum::middleware::from_fn(middleware::framing))
}

pub async fn serve(state: Shared, port: u16) {
    let address = SocketAddr::from((Ipv4Addr::UNSPECIFIED, port));

    let listener = match TcpListener::bind(address).await {
        Ok(listener) => listener,
        Err(failure) => {
            warn!("could not bind the transcript server on {port}; err = {failure}");

            return;
        }
    };

    if !site::BUILT {
        warn!("this build has no site");
    }

    match state.oauth.is_some() {
        true => info!("serving transcripts and the dashboard on {address}"),
        false => info!(
            "serving transcripts on {address}; the dashboard is off as no Discord application is configured"
        ),
    }

    if let Err(failure) = axum::serve(listener, router(state)).await {
        warn!("transcript server stopped; err = {failure}");
    }
}
