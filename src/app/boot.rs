use std::sync::Arc;
use std::time::Duration;

use tokio::time::sleep;
use tracing::{error, info, warn};

use crate::app::{App, config, shutdown};
use crate::command::registry::Registry;
use crate::features::punishments::lifecycle;
use crate::features::{self, errorlog};
use crate::platform::db::{self, retention};
use crate::platform::discord::dispatch::Dispatch;
use crate::platform::discord::gateway;
use crate::platform::discord::interact::Router;
use crate::platform::ocr;

#[cfg(feature = "web")]
use crate::web::oauth::Oauth;

#[derive(Debug, thiserror::Error)]
pub enum Failure {
    #[error(transparent)]
    Config(#[from] config::Failure),
    #[error("could not reach the database: {0}")]
    Pool(#[from] sqlx::Error),
    #[error(transparent)]
    Migrate(#[from] db::migrate::Failure),
    #[error("could not start the gateway: {0}")]
    Gateway(#[from] Box<serenity::Error>),
}

pub async fn run() -> Result<(), Failure> {
    let _ = rustls::crypto::aws_lc_rs::default_provider().install_default();

    let config = config::load()?;
    let pool = db::pool::connect(&config).await?;

    db::migrate::run(&pool).await?;

    let mut registry = Registry::new();

    features::register(&mut registry);
    info!("registered {} commands", registry.len());

    let mut controls = Router::new();

    features::control(&mut controls);
    info!("controlling {:?}", controls.keys());

    let app = Arc::new(App::new(pool, config, registry, controls));
    let mut dispatch = Dispatch::new();

    features::observe(&mut dispatch);

    info!("observing {:?}", dispatch.names());

    let token = app.config.token.clone();
    let mut client = gateway::build(Arc::clone(&app), dispatch, &token).await?;

    let _ = app.shards.set(Arc::clone(&client.shard_manager.runners));

    #[cfg(feature = "web")]
    if let Some(port) = app.config.web_port {
        use crate::web::{self, session::Sessions};

        tokio::spawn(web::serve(
            Arc::new(web::Web {
                pool: app.pool.clone(),
                keys: Arc::new(web::Custody {
                    pool: app.pool.clone(),
                    secrets: Arc::clone(&app.secrets),
                    http: Arc::clone(&client.http),
                }),
                guilds: Arc::new(web::directory::Present {
                    http: Arc::clone(&client.http),
                    cache: Arc::clone(&client.cache),
                }),
                rules: Arc::clone(&app.rules),
                settings: Arc::clone(&app.settings),
                permits: Arc::clone(&app.permits),
                sessions: Sessions::new(app.pool.clone()),
                client: app.http.client().clone(),
                oauth: registered(&app.config),
                site: app.config.site(),
                developers: app.config.dev_ids.clone().unwrap_or_default(),
            }),
            port,
        ));
    }

    tokio::spawn(ocr::warm());

    tokio::spawn({
        let app = Arc::clone(&app);

        async move {
            let mut ticks: u64 = 0;

            loop {
                sleep(Duration::from_secs(60)).await;

                app.reporter.flush().await;
                app.pending.sweep();
                app.attributed.sweep();
                app.awaiting.sweep();
                app.readings.sweep();
                app.notices.sweep();
                ocr::sweep();

                if ticks.is_multiple_of(60)
                    && let Err(failure) = retain(&app).await
                {
                    warn!("retention did not run; err = {failure}");
                }

                ticks += 1;
            }
        }
    });
    tokio::spawn(lifecycle::supervise(
        Arc::clone(&app),
        Arc::clone(&client.http),
    ));

    let stop_shards = client.shard_manager.get_shutdown_trigger();

    tokio::select! {
        outcome = client.start() => {
            if let Err(err) = outcome {
                error!("gateway stopped; err = {err:?}");
            }
        },
        _ = shutdown::requested_or_signalled(&app.stopping) => {
            stop_shards();
        },
    }

    app.reporter.flush().await;

    Ok(())
}

#[cfg(feature = "web")]
fn registered(config: &config::Environment) -> Option<Oauth> {
    let provided = |value: &Option<String>| {
        value
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(String::from)
    };

    Some(Oauth {
        client_id: provided(&config.discord_client_id)?,
        client_secret: provided(&config.discord_client_secret)?,
        redirect: format!("{}{}", config.site(), crate::web::CALLBACK),
    })
}

async fn retain(app: &App) -> Result<(), crate::command::error::Error> {
    retention::ensure(&app.pool).await?;
    retention::sweep(&app.pool).await?;

    let partitions = retention::prune(&app.pool).await?;

    if partitions > 0 {
        info!("dropped {partitions} expired message partitions");
    }

    errorlog::store::prune(&app.pool, 30).await?;

    Ok(())
}
