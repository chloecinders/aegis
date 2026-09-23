pub mod boot;
pub mod config;
pub mod shutdown;
#[cfg(feature = "self-update")]
pub mod updater;

use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant};

use dashmap::DashMap;
use serenity::all::{ShardId, ShardRunnerMetadata};
use sqlx::PgPool;

use crate::app::config::Environment;
use crate::command::registry::Registry;
use crate::domain::Snowflake;
use crate::domain::ids::RuleId;
use crate::features::archive::Storable;
use crate::features::archive::cache::Recent;
use crate::features::archive::secrets::Keys;
use crate::features::archive::store as archive;
use crate::features::automod::cache::Rules;
use crate::features::automod::eval::Strikes;
use crate::features::automod::readings::Readings;
use crate::features::diagnostics::store::{self, TraceRow};
use crate::features::guildlog::amend::Awaiting;
use crate::features::guildlog::poller::{Count, Reader};
use crate::features::permissions::cache::Permits;
use crate::features::punishments::notices::Notices;
use crate::features::settings::cache::Settings;
use crate::features::sticky::cache::Stickies;
use crate::platform::cache::Debounce;
use crate::platform::db::writer::Batched;
use crate::platform::discord::interact::Router;
use crate::platform::discord::pending::Pending;
use crate::platform::http::Http;
use crate::platform::observe::report::Reporter;

pub struct App {
    pub pool: PgPool,
    pub config: Environment,
    pub http: Http,
    pub reporter: Reporter,
    pub pending: Pending,
    pub registry: Registry,
    pub controls: Router,
    pub traces: Batched<TraceRow>,
    pub settings: Arc<Settings>,
    pub stickies: Stickies,
    pub permits: Arc<Permits>,
    pub audit: Reader,
    pub attributed: Count,
    pub awaiting: Awaiting,
    pub rules: Arc<Rules>,
    pub strikes: Strikes,
    pub punished: Debounce<(RuleId, Snowflake, Snowflake)>,
    pub readings: Readings,
    pub notices: Notices,
    pub recent: Recent,
    pub messages: Batched<Storable>,
    pub secrets: Arc<Keys>,
    pub stopping: shutdown::Requested,
    pub started_at: Instant,
    pub shards: OnceLock<Arc<DashMap<ShardId, ShardRunnerMetadata>>>,
}

impl App {
    pub fn new(pool: PgPool, config: Environment, registry: Registry, controls: Router) -> Self {
        let pool_for_traces = pool.clone();
        let pool_for_messages = pool.clone();
        let http = Http::new();
        let reporter = Reporter::new(http.clone(), config.webhook.clone());

        let window = config
            .edit_window()
            .to_std()
            .unwrap_or(Duration::from_secs(300));

        Self {
            pool,
            config,
            http,
            reporter,
            pending: Pending::new(),
            registry,
            controls,
            traces: store::sink(pool_for_traces),
            settings: Arc::new(Settings::new()),
            stickies: Stickies::new(),
            permits: Arc::new(Permits::new()),
            audit: Reader::new(),
            attributed: Count::new(),
            awaiting: Awaiting::new(),
            rules: Arc::new(Rules::new()),
            strikes: Strikes::new(8192),
            punished: Debounce::new(4096, Duration::from_secs(15)),
            readings: Readings::new(),
            notices: Notices::new(window),
            recent: Recent::new(),
            messages: archive::sink(pool_for_messages),
            secrets: Arc::new(Keys::new()),
            stopping: shutdown::Requested::new(),
            started_at: Instant::now(),
            shards: OnceLock::new(),
        }
    }

    pub fn shard_latency(&self, shard: ShardId) -> Option<Duration> {
        let runner = self.shards.get()?.get(&shard)?;

        runner.info.read().latency
    }

    pub fn prefix(&self) -> &str {
        &self.config.prefix
    }

    pub fn uptime(&self) -> Duration {
        self.started_at.elapsed()
    }

    pub fn is_developer(&self, user: Snowflake) -> bool {
        self.config.is_developer(user)
    }

    pub fn allows_guild(&self, guild: Snowflake) -> bool {
        self.config.allows_guild(guild)
    }
}
