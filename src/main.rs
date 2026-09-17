mod app;
mod command;
mod domain;
mod features;
mod platform;
#[cfg(feature = "web")]
mod web;

use tracing_subscriber::EnvFilter;

#[cfg(not(target_env = "msvc"))]
#[global_allocator]
static ALLOC: tikv_jemallocator::Jemalloc = tikv_jemallocator::Jemalloc;

#[cfg(not(target_env = "msvc"))]
#[unsafe(export_name = "_rjem_malloc_conf")]
static JEMALLOC_CONF: &[u8; 73] =
    b"background_thread:true,narenas:4,dirty_decay_ms:5000,muzzy_decay_ms:5000\0";

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    #[cfg(feature = "self-update")]
    if app::updater::intercept() {
        return;
    }

    if features::help::dump::intercept() {
        return;
    }

    if let Err(failure) = app::boot::run().await {
        tracing::error!("{failure}");
        std::process::exit(1);
    }
}
