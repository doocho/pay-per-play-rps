use rust_decimal::Decimal;
use sqlx::PgExecutor;
use uuid::Uuid;

use crate::types::pvp::PvpPaymentRow;

pub struct CreatePvpPaymentParams {
    pub pvp_game_id: Uuid,
    pub player_id: Uuid,
    pub amount: Decimal,
    pub provider_payment_id: Option<String>,
    pub authorization_payload: Option<serde_json::Value>,
    pub receipt_payload: Option<serde_json::Value>,
}

pub async fn create(
    executor: impl PgExecutor<'_>,
    params: CreatePvpPaymentParams,
) -> Result<PvpPaymentRow, sqlx::Error> {
    sqlx::query_as::<_, PvpPaymentRow>(
        r#"
        INSERT INTO pvp_payments (pvp_game_id, player_id, amount, provider_payment_id, authorization_payload, receipt_payload)
        VALUES ($1, $2, $3, $4, $5, $6)
        RETURNING *
        "#,
    )
    .bind(params.pvp_game_id)
    .bind(params.player_id)
    .bind(params.amount)
    .bind(&params.provider_payment_id)
    .bind(&params.authorization_payload)
    .bind(&params.receipt_payload)
    .fetch_one(executor)
    .await
}

pub async fn find_by_game_and_player(
    executor: impl PgExecutor<'_>,
    pvp_game_id: Uuid,
    player_id: Uuid,
) -> Result<Option<PvpPaymentRow>, sqlx::Error> {
    sqlx::query_as::<_, PvpPaymentRow>(
        "SELECT * FROM pvp_payments WHERE pvp_game_id = $1 AND player_id = $2",
    )
    .bind(pvp_game_id)
    .bind(player_id)
    .fetch_optional(executor)
    .await
}

pub async fn count_for_game(
    executor: impl PgExecutor<'_>,
    pvp_game_id: Uuid,
) -> Result<i64, sqlx::Error> {
    let row: (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM pvp_payments WHERE pvp_game_id = $1")
            .bind(pvp_game_id)
            .fetch_one(executor)
            .await?;
    Ok(row.0)
}
