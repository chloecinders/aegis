use std::{
    collections::HashMap,
    env, fs, panic,
    sync::Arc,
    time::{Duration, Instant},
};

use serenity::{
    Client,
    all::{GatewayIntents, Settings, ShardManager},
    prelude::TypeMapKey,
};
use sqlx::{PgPool, postgres::PgPoolOptions};
use tokio::{fs::File, io::AsyncReadExt, sync::Mutex, time::sleep};
use tracing::error;

use crate::{
    auto_once::AutoOnceLock,
    config::{Config, Environment},
    event_handler::Handler,
    utils::{GuildSettings, consume_pgsql_error, send_error},
};
use std::process::Command as SystemCommand;

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
    tracing_subscriber::fmt::fmt().init();

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

    #[cfg(unix)]
    let mut sigterm = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
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
