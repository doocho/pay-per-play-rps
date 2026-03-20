use rust_decimal::Decimal;
use sqlx::PgExecutor;
use uuid::Uuid;

use crate::types::domain::PaymentRow;

pub struct CreatePaymentParams {
    pub game_id: Uuid,
    pub amount: Decimal,
    pub provider_payment_id: Option<String>,
    pub authorization_payload: Option<serde_json::Value>,
    pub receipt_payload: Option<serde_json::Value>,
}

pub async fn create(
    tx: impl PgExecutor<'_>,
    params: CreatePaymentParams,
) -> Result<PaymentRow, sqlx::Error> {
    sqlx::query_as::<_, PaymentRow>(
        r#"
        INSERT INTO payments (game_id, amount, provider_payment_id, authorization_payload, receipt_payload)
        VALUES ($1, $2, $3, $4, $5)
        RETURNING *
        "#,
    )
    .bind(params.game_id)
    .bind(params.amount)
    .bind(&params.provider_payment_id)
    .bind(&params.authorization_payload)
    .bind(&params.receipt_payload)
    .fetch_one(tx)
    .await
}

pub async fn find_by_game_id(
    tx: impl PgExecutor<'_>,
    game_id: Uuid,
) -> Result<Option<PaymentRow>, sqlx::Error> {
    sqlx::query_as::<_, PaymentRow>("SELECT * FROM payments WHERE game_id = $1")
        .bind(game_id)
        .fetch_optional(tx)
        .await
}
