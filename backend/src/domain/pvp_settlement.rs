use rust_decimal::Decimal;
use sqlx::PgPool;
use uuid::Uuid;

use crate::db;
use crate::domain::pvp_game;
use crate::error::AppError;
use crate::types::domain::TokenType;
use crate::types::pvp::{PvpGameRow, PvpGameStatus, PvpOutcome};

pub struct PvpSettlementPlan {
    pub result: PvpOutcome,
    pub pot_amount: Decimal,
    pub platform_fee: Decimal,
    pub winner_payout: Decimal,
    pub loser_refund: Decimal,
    pub winner_id: Option<Uuid>,
    pub reward_token: Option<TokenType>,
    pub reward_amount: i32,
}

impl PvpSettlementPlan {
    pub fn from_game(game: &PvpGameRow, outcome: &PvpOutcome) -> Self {
        let pot = game.price * Decimal::new(2, 0);
        let fee_bps = Decimal::new(game.platform_fee_bps as i64, 0);
        let platform_fee = pot * fee_bps / Decimal::new(10000, 0);

        match outcome {
            PvpOutcome::Player1Wins | PvpOutcome::Player2Wins => {
                let winner_payout = pot - platform_fee;
                let w_id = pvp_game::winner_id(game, outcome);
                let w_choice = pvp_game::winner_choice(game, outcome);
                Self {
                    result: *outcome,
                    pot_amount: pot,
                    platform_fee,
                    winner_payout,
                    loser_refund: Decimal::ZERO,
                    winner_id: w_id,
                    reward_token: w_choice.map(|c| c.into()),
                    reward_amount: 1,
                }
            }
            PvpOutcome::Draw => {
                // Draw after max rematch rounds: refund both players (minus platform fee split)
                let per_player_refund = (pot - platform_fee) / Decimal::new(2, 0);
                Self {
                    result: *outcome,
                    pot_amount: pot,
                    platform_fee,
                    winner_payout: Decimal::ZERO,
                    loser_refund: per_player_refund,
                    winner_id: None,
                    reward_token: None,
                    reward_amount: 0,
                }
            }
        }
    }
}

/// Execute full PvP settlement inside a transaction.
pub async fn execute(pool: &PgPool, game: &PvpGameRow) -> Result<Uuid, AppError> {
    let outcome = game
        .result
        .ok_or_else(|| AppError::Internal(anyhow::anyhow!("pvp game has no result")))?;

    let plan = PvpSettlementPlan::from_game(game, &outcome);

    let mut tx = pool.begin().await?;

    // Transition to settling
    db::pvp_games::transition(&mut *tx, game.id, PvpGameStatus::Settling).await?;

    // Create settlement record
    let settlement = db::pvp_settlements::create(
        &mut *tx,
        db::pvp_settlements::CreatePvpSettlementParams {
            pvp_game_id: game.id,
            result: plan.result,
            pot_amount: plan.pot_amount,
            platform_fee: plan.platform_fee,
            winner_payout: plan.winner_payout,
            loser_refund: plan.loser_refund,
            winner_id: plan.winner_id,
            reward_token: plan.reward_token,
            reward_amount: plan.reward_amount,
        },
    )
    .await?;

    // Credit inventory for winner
    if let (Some(token), Some(winner)) = (plan.reward_token, plan.winner_id) {
        if plan.reward_amount > 0 {
            db::inventories::credit(&mut *tx, winner, token, plan.reward_amount).await?;
        }
    }

    // Mark settled
    db::pvp_games::mark_settled(&mut *tx, game.id).await?;

    // Clean up matchmaking queue entry if any
    db::matchmaking::remove_by_game(&mut *tx, game.id).await?;

    tx.commit().await?;

    Ok(settlement.id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_settlement_plan_winner() {
        // Create a minimal mock game row for testing
        let game = PvpGameRow {
            id: Uuid::new_v4(),
            room_code: None,
            player1_id: Some(Uuid::new_v4()),
            player2_id: Some(Uuid::new_v4()),
            status: PvpGameStatus::ResolvedPlayer1Wins,
            price: Decimal::new(5, 2), // 0.05
            currency: "USD".to_string(),
            platform_fee_bps: 500, // 5%
            player1_choice: Some(crate::types::domain::Choice::Rock),
            player1_salt: None,
            player1_commit: None,
            player2_choice: Some(crate::types::domain::Choice::Scissors),
            player2_salt: None,
            player2_commit: None,
            result: Some(PvpOutcome::Player1Wins),
            rounds: serde_json::Value::Array(vec![]),
            current_round: 1,
            created_at: chrono::Utc::now(),
            player2_joined_at: None,
            both_paid_at: None,
            both_committed_at: None,
            resolved_at: None,
            settled_at: None,
        };

        let plan = PvpSettlementPlan::from_game(&game, &PvpOutcome::Player1Wins);

        // pot = 0.05 * 2 = 0.10
        assert_eq!(plan.pot_amount, Decimal::new(10, 2));
        // fee = 0.10 * 500 / 10000 = 0.005
        assert_eq!(plan.platform_fee, Decimal::new(5, 3));
        // payout = 0.10 - 0.005 = 0.095
        assert_eq!(plan.winner_payout, Decimal::new(95, 3));
        assert_eq!(plan.loser_refund, Decimal::ZERO);
        assert_eq!(plan.winner_id, game.player1_id);
        assert_eq!(
            plan.reward_token,
            Some(crate::types::domain::TokenType::Rock)
        );
    }

    #[test]
    fn test_settlement_plan_draw() {
        let game = PvpGameRow {
            id: Uuid::new_v4(),
            room_code: None,
            player1_id: Some(Uuid::new_v4()),
            player2_id: Some(Uuid::new_v4()),
            status: PvpGameStatus::ResolvedDraw,
            price: Decimal::new(5, 2),
            currency: "USD".to_string(),
            platform_fee_bps: 500,
            player1_choice: Some(crate::types::domain::Choice::Rock),
            player1_salt: None,
            player1_commit: None,
            player2_choice: Some(crate::types::domain::Choice::Rock),
            player2_salt: None,
            player2_commit: None,
            result: Some(PvpOutcome::Draw),
            rounds: serde_json::Value::Array(vec![]),
            current_round: 10,
            created_at: chrono::Utc::now(),
            player2_joined_at: None,
            both_paid_at: None,
            both_committed_at: None,
            resolved_at: None,
            settled_at: None,
        };

        let plan = PvpSettlementPlan::from_game(&game, &PvpOutcome::Draw);

        assert_eq!(plan.pot_amount, Decimal::new(10, 2));
        assert_eq!(plan.winner_payout, Decimal::ZERO);
        assert!(plan.winner_id.is_none());
        assert!(plan.reward_token.is_none());
        // Each player gets back (pot - fee) / 2 = (0.10 - 0.005) / 2 = 0.0475
        assert_eq!(plan.loser_refund, Decimal::new(475, 4));
    }
}
