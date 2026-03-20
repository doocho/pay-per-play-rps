use sqlx::PgExecutor;
use uuid::Uuid;

use crate::types::domain::{InventoryRow, TokenType};

pub async fn credit(
    tx: impl PgExecutor<'_>,
    user_id: Uuid,
    token: TokenType,
    amount: i32,
) -> Result<InventoryRow, sqlx::Error> {
    sqlx::query_as::<_, InventoryRow>(
        r#"
        INSERT INTO inventories (user_id, token_type, balance, updated_at)
        VALUES ($1, $2, $3, now())
        ON CONFLICT (user_id, token_type) DO UPDATE
            SET balance = inventories.balance + $3,
                updated_at = now()
        RETURNING *
        "#,
    )
    .bind(user_id)
    .bind(&token)
    .bind(amount)
    .fetch_one(tx)
    .await
}

pub async fn find_by_user(
    tx: impl PgExecutor<'_>,
    user_id: Uuid,
) -> Result<Vec<InventoryRow>, sqlx::Error> {
    sqlx::query_as::<_, InventoryRow>(
        "SELECT * FROM inventories WHERE user_id = $1 ORDER BY token_type",
    )
    .bind(user_id)
    .fetch_all(tx)
    .await
}

pub async fn find_by_wallet(
    tx: impl PgExecutor<'_>,
    wallet_address: &str,
) -> Result<Vec<InventoryRow>, sqlx::Error> {
    sqlx::query_as::<_, InventoryRow>(
        r#"
        SELECT i.* FROM inventories i
        JOIN users u ON u.id = i.user_id
        WHERE u.wallet_address = $1
        ORDER BY i.token_type
        "#,
    )
    .bind(wallet_address)
    .fetch_all(tx)
    .await
}
