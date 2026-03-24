use std::sync::Arc;

use axum::Router;
use mpp::server::axum::ChargeChallenger;
use mpp::server::TempoProvider;
use sqlx::PgPool;

use crate::config::AppConfig;

pub struct AppState {
    pub db: PgPool,
    pub mpp: Arc<dyn ChargeChallenger>,
    /// Same RPC as MPP — used to map payment `Receipt.reference` (tx hash) → payer address.
    pub tempo_provider: TempoProvider,
    pub config: AppConfig,
}

pub fn create_router(state: Arc<AppState>) -> Router {
    let api = Router::new()
        .merge(crate::routes::play::router())
        .merge(crate::routes::games::router())
        .merge(crate::routes::receipts::router())
        .merge(crate::routes::fairness::router())
        .merge(crate::routes::leaderboard::router())
        .merge(crate::routes::inventory::router())
        .merge(crate::routes::health::router())
        .merge(crate::routes::pvp::router())
        .merge(crate::routes::pvp_fairness::router());

    Router::new()
        .merge(crate::routes::llms::router())
        .nest("/api", api)
        .layer(crate::middleware::request_id::layer())
        .layer(
            tower_http::trace::TraceLayer::new_for_http().make_span_with(
                tower_http::trace::DefaultMakeSpan::new().level(tracing::Level::INFO),
            ),
        )
        .layer(
            tower_http::cors::CorsLayer::new()
                .allow_origin(tower_http::cors::Any)
                .allow_methods(tower_http::cors::Any)
                .allow_headers(tower_http::cors::Any),
        )
        .with_state(state)
}
