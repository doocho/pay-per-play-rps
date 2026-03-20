use sqlx::PgPool;
use uuid::Uuid;

use crate::types::domain::UserRow;

pub async fn upsert_by_wallet(pool: &PgPool, wallet_address: &str) -> Result<UserRow, sqlx::Error> {
    sqlx::query_as::<_, UserRow>(
        r#"
        INSERT INTO users (wallet_address)
        VALUES ($1)
        ON CONFLICT (wallet_address) DO UPDATE SET wallet_address = EXCLUDED.wallet_address
        RETURNING id, wallet_address, created_at
        "#,
    )
    .bind(wallet_address)
    .fetch_one(pool)
    .await
}

pub async fn find_by_wallet(
    pool: &PgPool,
    wallet_address: &str,
) -> Result<Option<UserRow>, sqlx::Error> {
    sqlx::query_as::<_, UserRow>(
        "SELECT id, wallet_address, created_at FROM users WHERE wallet_address = $1",
    )
    .bind(wallet_address)
    .fetch_optional(pool)
    .await
}

pub async fn find_by_id(pool: &PgPool, id: Uuid) -> Result<Option<UserRow>, sqlx::Error> {
    sqlx::query_as::<_, UserRow>(
        "SELECT id, wallet_address, created_at FROM users WHERE id = $1",
    )
    .bind(id)
    .fetch_optional(pool)
    .await
}
