use rust_decimal::Decimal;
use sqlx::PgExecutor;
use uuid::Uuid;

use crate::types::domain::TokenType;
use crate::types::pvp::{PvpOutcome, PvpSettlementRow};

pub struct CreatePvpSettlementParams {
    pub pvp_game_id: Uuid,
    pub result: PvpOutcome,
    pub pot_amount: Decimal,
    pub platform_fee: Decimal,
    pub winner_payout: Decimal,
    pub loser_refund: Decimal,
    pub winner_id: Option<Uuid>,
    pub reward_token: Option<TokenType>,
    pub reward_amount: i32,
}

pub async fn create(
    executor: impl PgExecutor<'_>,
    params: CreatePvpSettlementParams,
) -> Result<PvpSettlementRow, sqlx::Error> {
    sqlx::query_as::<_, PvpSettlementRow>(
        r#"
        INSERT INTO pvp_settlements (
            pvp_game_id, result, pot_amount, platform_fee, winner_payout,
            loser_refund, winner_id, reward_token, reward_amount
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
        RETURNING *
        "#,
    )
    .bind(params.pvp_game_id)
    .bind(&params.result)
    .bind(params.pot_amount)
    .bind(params.platform_fee)
    .bind(params.winner_payout)
    .bind(params.loser_refund)
    .bind(params.winner_id)
    .bind(&params.reward_token)
    .bind(params.reward_amount)
    .fetch_one(executor)
    .await
}

pub async fn find_by_game_id(
    executor: impl PgExecutor<'_>,
    pvp_game_id: Uuid,
) -> Result<Option<PvpSettlementRow>, sqlx::Error> {
    sqlx::query_as::<_, PvpSettlementRow>(
        "SELECT * FROM pvp_settlements WHERE pvp_game_id = $1",
    )
    .bind(pvp_game_id)
    .fetch_optional(executor)
    .await
}
