use sqlx::PgPool;
use uuid::Uuid;

use crate::db;
use crate::error::AppError;
use crate::types::domain::InventoryRow;

pub async fn get_balances_for_wallet(
    pool: &PgPool,
    wallet_address: &str,
) -> Result<Vec<InventoryRow>, AppError> {
    let rows = db::inventories::find_by_wallet(pool, wallet_address).await?;
    Ok(rows)
}

pub async fn get_balances_for_user(
    pool: &PgPool,
    user_id: Uuid,
) -> Result<Vec<InventoryRow>, AppError> {
    let rows = db::inventories::find_by_user(pool, user_id).await?;
    Ok(rows)
}
