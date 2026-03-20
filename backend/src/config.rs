use rust_decimal::Decimal;
use std::str::FromStr;

#[derive(Debug, Clone)]
pub struct AppConfig {
    pub database_url: String,
    pub play_price: Decimal,
    pub play_currency: String,
    pub game_ttl_seconds: u64,
    pub idempotency_ttl_seconds: u64,
    pub mpp_secret_key: String,
    pub mpp_recipient: String,
    pub mpp_rpc_url: String,
    pub mpp_realm: String,
    pub port: u16,
}

impl AppConfig {
    pub fn from_env() -> Result<Self, anyhow::Error> {
        Ok(Self {
            database_url: require_env("DATABASE_URL")?,
            play_price: Decimal::from_str(&env_or("PLAY_PRICE", "0.05"))?,
            play_currency: env_or("PLAY_CURRENCY", "USD"),
            game_ttl_seconds: env_or("GAME_TTL_SECONDS", "300").parse()?,
            idempotency_ttl_seconds: env_or("IDEMPOTENCY_TTL_SECONDS", "86400").parse()?,
            mpp_secret_key: require_env("MPP_SECRET_KEY")?,
            mpp_recipient: require_env("MPP_RECIPIENT")?,
            mpp_rpc_url: env_or("MPP_RPC_URL", "https://rpc.moderato.tempo.xyz"),
            mpp_realm: env_or("MPP_REALM", "pay-per-play-rps"),
            port: env_or("PORT", "8080").parse()?,
        })
    }

    pub fn play_price_str(&self) -> String {
        self.play_price.to_string()
    }
}

fn require_env(key: &str) -> Result<String, anyhow::Error> {
    std::env::var(key).map_err(|_| anyhow::anyhow!("missing required env var: {key}"))
}

fn env_or(key: &str, default: &str) -> String {
    std::env::var(key).unwrap_or_else(|_| default.to_string())
}
