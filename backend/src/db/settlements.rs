use rust_decimal::Decimal;
use sqlx::PgExecutor;
use uuid::Uuid;

use crate::types::domain::{Outcome, SettlementRow, TokenType};

pub struct CreateSettlementParams {
    pub game_id: Uuid,
    pub outcome: Outcome,
    pub refund_amount: Decimal,
    pub captured_amount: Decimal,
    pub reward_token: Option<TokenType>,
    pub reward_amount: i32,
}

pub async fn create(
    tx: impl PgExecutor<'_>,
    params: CreateSettlementParams,
) -> Result<SettlementRow, sqlx::Error> {
    sqlx::query_as::<_, SettlementRow>(
        r#"
        INSERT INTO settlements (game_id, outcome, refund_amount, captured_amount, reward_token, reward_amount)
        VALUES ($1, $2, $3, $4, $5, $6)
        RETURNING *
        "#,
    )
    .bind(params.game_id)
    .bind(&params.outcome)
    .bind(params.refund_amount)
    .bind(params.captured_amount)
    .bind(&params.reward_token)
    .bind(params.reward_amount)
    .fetch_one(tx)
    .await
}

pub async fn find_by_game_id(
    tx: impl PgExecutor<'_>,
    game_id: Uuid,
) -> Result<Option<SettlementRow>, sqlx::Error> {
    sqlx::query_as::<_, SettlementRow>("SELECT * FROM settlements WHERE game_id = $1")
        .bind(game_id)
        .fetch_optional(tx)
        .await
}
