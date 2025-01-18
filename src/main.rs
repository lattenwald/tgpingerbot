use clap::Parser;
use tracing::info;
use tracing_subscriber::EnvFilter;

use tgpingbot::{bot::start_bot, config::Args, storage::Storage};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = Args::parse().get_config();
    dbg!(&config);

    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_env("LOG_LEVEL"))
        .with_target(false)
        .init();
    info!("Logger ok");

    let storage = Storage::init(&config.storage).await.unwrap();
    info!("Storage ok");

    // let mut bot_handle = init_bot(config.token).await;
    // bot_handle.dispatch().await;
    start_bot(config.token, storage).await;
    info!("bot started");

    Ok(())
}
