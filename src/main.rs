use std::{
    collections::HashMap,
    env, fs, panic,
    path::Path,
    sync::Arc,
    time::{Duration, Instant},
};

use axum::{extract::Request, middleware::Next, response::Response};
use serenity::{
    Client,
    all::{GatewayIntents, Settings, ShardManager},
    prelude::TypeMapKey,
};
use sqlx::{PgPool, postgres::PgPoolOptions};
use tokio::{fs::File, io::AsyncReadExt, net::TcpListener, sync::Mutex, time::sleep};
use tracing::error;

use crate::{
    auto_once::AutoOnceLock,
    config::{Config, Environment},
    event_handler::Handler,
    utils::{GuildSettings, consume_pgsql_error, send_error},
};
use std::process::Command as SystemCommand;

use axum::{
    Json, Router,
    extract::Path as AxumPath,
    http::StatusCode,
    middleware,
    response::{Html, IntoResponse},
    routing::get,
};
use tower_http::services::ServeDir;

#[cfg(unix)]
use tokio::signal::unix::SignalKind;

pub struct ShardManagerContainer;

impl TypeMapKey for ShardManagerContainer {
    type Value = Arc<ShardManager>;
}

mod auto_once;
mod commands;
mod config;
mod constants;
mod database;
mod event_handler;
mod lexer;
mod moderation;
mod tasks;
mod transformers;
mod utils;

#[cfg(not(target_env = "msvc"))]
#[global_allocator]
static ALLOC: tikv_jemallocator::Jemalloc = tikv_jemallocator::Jemalloc;

pub static START_TIME: AutoOnceLock<Instant> = AutoOnceLock::new();
pub static SQL: AutoOnceLock<PgPool> = AutoOnceLock::new();
pub static GUILD_SETTINGS: AutoOnceLock<Mutex<GuildSettings>> = AutoOnceLock::new();
pub static BOT_CONFIG: AutoOnceLock<Environment> = AutoOnceLock::new();
pub static ENCRYPTION_KEYS: AutoOnceLock<Mutex<HashMap<u64, [u8; 32]>>> = AutoOnceLock::new();

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    #[cfg(target_os = "windows")]
    if let Some(arg) = std::env::args()
        .collect::<Vec<String>>()
        .iter()
        .find(|a| a.starts_with("--update"))
    {
        use std::process::exit;
        use tracing::info;

        info!("Starting update process");
        if let Err(err) = update(arg) {
            send_error(String::from("UPDATE ERROR"), err.to_string());
        }
        exit(0);
    }

    if let Err(err) = cleanup() {
        send_error(String::from("UPDATE CLEANUP ERROR"), err.to_string());
    };

    let _ = START_TIME.set(Instant::now());

    let mut file = File::open("./Config.toml")
        .await
        .expect("Could not find Config.toml in project root.");
    let mut contents = String::new();

    if file.read_to_string(&mut contents).await.is_err() {
        panic!("Could not read Config.toml.");
    }

    let config: Config = toml::from_str(contents.as_str())
        .unwrap_or_else(|_| panic!("Could not parse Config.toml."));

    let active_env = match config.bot.env.as_str() {
        "release" => &config.release,
        "dev" => &config.dev.expect("You need to add a dev environment in the config if you are gonna specify to use th dev environment..."),
        _ => panic!("Unknown bot.env, verify bot.env is one of release or dev"),
    };

    let _ = SQL.set({
        async {
            PgPoolOptions::new()
                .max_connections(active_env.max_connections.unwrap_or(5))
                .connect(&active_env.database_url)
                .await
                .expect("Failed to create database pool, make sure the database url in the config is valid.")
        }.await
    });

    GUILD_SETTINGS
        .set(Mutex::new(GuildSettings::new()))
        .unwrap();

    BOT_CONFIG.set(active_env.clone()).unwrap();
    ENCRYPTION_KEYS.set(Mutex::new(HashMap::new())).unwrap();

    if let Err(err) = sqlx::migrate!().run(&*SQL).await {
        let dbg = format!("{err:?}");
        consume_pgsql_error("MIGRATIONS".into(), err.into());
        panic!("Could not run database migrations: {dbg}");
    }

    panic::set_hook(Box::new(|info| {
        let payload_str = if let Some(s) = info.payload().downcast_ref::<&str>() {
            Some(s.to_string())
        } else if let Some(s) = info.payload().downcast_ref::<String>() {
            Some(s.clone())
        } else {
            None
        };

        send_error(
            String::from("Thread Panic"),
            format!("Panic info: {info:?}; Payload: {payload_str:?}"),
        );
    }));

    let intents = GatewayIntents::all();

    let mut cache_settings = Settings::default();
    cache_settings.max_messages = 0;
    let handler = Handler::new(active_env.prefix.clone());

    let mut client = Client::builder(&active_env.token, intents)
        .event_handler(handler)
        .cache_settings(cache_settings)
        .await
        .expect("Unable to create client");

    let shard_manager = client.shard_manager.clone();
    client
        .data
        .write()
        .await
        .insert::<ShardManagerContainer>(shard_manager);

    let http = client.http.clone();

    tokio::spawn(async move {
        loop {
            sleep(Duration::from_secs(60 * 5)).await;
            tasks::check_expiring_bans(&http).await;
            tasks::check_expiring_timeouts(&http).await;
        }
    });

    let web_port = active_env.web_port.unwrap_or(3000);
    tokio::spawn(async move {
        let app = Router::new()
            .route(
                "/transcript/:guild_id/:transcript_id",
                get(transcript_page_handler),
            )
            .route(
                "/api/transcript/:guild_id/:transcript_id",
                get(transcript_api_handler),
            )
            .fallback_service(ServeDir::new("website/dist"))
            .layer(middleware::from_fn(clean_url_middleware));
        let bind_addr = format!("0.0.0.0:{web_port}");

        match TcpListener::bind(&bind_addr).await {
            Ok(listener) => {
                tracing::info!(
                    "Embedded web server listening on http://{bind_addr} (serving website/dist)"
                );
                if let Err(e) = axum::serve(listener, app).await {
                    tracing::error!("Embedded web server error: {e:?}");
                }
            }
            Err(e) => {
                tracing::error!("Failed to bind web server port {bind_addr}: {e:?}");
            }
        }
    });

    #[cfg(unix)]
    let mut sigterm = tokio::signal::unix::signal(SignalKind::terminate())
        .expect("Failed to bind SIGTERM listener");

    tokio::select! {
        res = client.start() => {
            if let Err(e) = res {
                error!("Client error: {e:?}");
            }
        },
        _ = tokio::signal::ctrl_c() => {
            tracing::info!("Received Ctrl-C! Shutting down...");
        },
        _ = async {
            #[cfg(unix)]
            {
                sigterm.recv().await
            }
            #[cfg(not(unix))]
            {
                std::future::pending::<()>().await
            }
        } => {
            tracing::info!("Received SIGTERM! Shutting down...");
        }
    }
}

#[allow(unreachable_code, dead_code)]
fn update(arg: &str) -> std::io::Result<()> {
    let exe = env::current_exe()?;

    let name = "Aegis.exe";

    let mut target = exe.parent().unwrap().to_path_buf();
    target.push(name);

    if target.exists() {
        fs::remove_file(&target)?;
    }

    fs::copy(&exe, &target)?;

    let id = arg.split("=").last().unwrap_or("");

    match SystemCommand::new(format!(".{}{}", std::path::MAIN_SEPARATOR, name))
        .arg(format!("--id={id}"))
        .spawn()
    {
        Ok(c) => drop(c),
        Err(e) => error!("Could not spawn new process; err = {e:?}"),
    };

    Ok(())
}

async fn transcript_page_handler() -> impl IntoResponse {
    let html = r#"<!doctype html>
<html lang="en">
<head>
    <meta charset="UTF-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1.0" />
    <title>Transcript Viewer</title>
    <link rel="preconnect" href="https://fonts.googleapis.com" />
    <link rel="preconnect" href="https://fonts.gstatic.com" crossorigin />
    <link rel="stylesheet" href="https://fonts.googleapis.com/css2?family=Inter:wght@400;600;700&display=swap" />
    <link rel="stylesheet" href="/styles.css" />
    <style>
        body {
            background-color: #313338;
            margin: 0;
            padding: 0;
        }
        .discord-transcript-container {
            margin: 0;
            border: none;
            border-radius: 0;
            min-height: 100vh;
        }
    </style>
</head>
<body>
    <div id="app"></div>
    <script src="/transcript.js"></script>
</body>
</html>"#;
    Html(html)
}

async fn transcript_api_handler(
    AxumPath((guild_id, transcript_id)): AxumPath<(u64, String)>,
) -> impl IntoResponse {
    match utils::fetch_transcript_data(guild_id, &transcript_id).await {
        Some(data) => (StatusCode::OK, Json(data)).into_response(),
        None => (StatusCode::NOT_FOUND, "Transcript not found").into_response(),
    }
}

fn cleanup() -> std::io::Result<()> {
    let current_dir = std::env::current_dir()?;

    for entry in fs::read_dir(&current_dir)? {
        let entry = entry?;
        let path = entry.path();

        if path.is_file()
            && let Some(filename) = path.file_name().and_then(|f| f.to_str())
            && filename.starts_with("new_")
            && filename.contains("aegis")
        {
            fs::remove_file(&path)?;
        }
    }

    Ok(())
}

async fn clean_url_middleware(mut req: Request, next: Next) -> Response {
    let path = req.uri().path().trim_start_matches('/');

    if !path.is_empty() && !req.uri().path().contains('.') {
        let html_path = Path::new("website/dist").join(path).with_extension("html");

        if html_path.is_file() {
            let new_path = format!("/{path}.html");
            if let Ok(new_uri) = new_path.parse() {
                *req.uri_mut() = new_uri;
            }
        }
    }
    next.run(req).await
}
