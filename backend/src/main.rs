use std::sync::Arc;

use mpp::server::{tempo, Mpp, TempoConfig};
use sqlx::postgres::PgPoolOptions;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

mod app;
mod config;
mod db;
mod domain;
mod error;
mod jobs;
mod middleware;
mod routes;
mod types;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();

    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| {
            "pay_per_play_rps=debug,tower_http=debug,info".into()
        }))
        .with(tracing_subscriber::fmt::layer())
        .init();

    let config = config::AppConfig::from_env()?;

    let db = PgPoolOptions::new()
        .max_connections(10)
        .connect(&config.database_url)
        .await?;

    sqlx::migrate!("./migrations").run(&db).await?;

    let mpp = Mpp::create(
        tempo(TempoConfig {
            recipient: &config.mpp_recipient,
        })
        .realm(&config.mpp_realm)
        .secret_key(&config.mpp_secret_key)
        .rpc_url(&config.mpp_rpc_url),
    )?;

    let state = Arc::new(app::AppState {
        db: db.clone(),
        mpp: Arc::new(mpp) as Arc<dyn mpp::server::axum::ChargeChallenger>,
        config: config.clone(),
    });

    jobs::spawn_all(db, &config);

    let addr = format!("0.0.0.0:{}", config.port);
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    tracing::info!("listening on {addr}");

    axum::serve(listener, app::create_router(state)).await?;

    Ok(())
}
