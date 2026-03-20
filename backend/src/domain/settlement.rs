use rust_decimal::Decimal;
use sqlx::PgPool;
use uuid::Uuid;

use crate::db;
use crate::error::AppError;
use crate::types::domain::{Choice, GameRow, GameStatus, Outcome, TokenType};

pub struct SettlementPlan {
    pub outcome: Outcome,
    pub captured_amount: Decimal,
    pub refund_amount: Decimal,
    pub reward_token: Option<TokenType>,
    pub reward_amount: i32,
}

impl SettlementPlan {
    pub fn from_outcome(outcome: &Outcome, price: Decimal, user_choice: &Choice) -> Self {
        match outcome {
            Outcome::Win => Self {
                outcome: *outcome,
                captured_amount: price,
                refund_amount: Decimal::ZERO,
                reward_token: Some((*user_choice).into()),
                reward_amount: 1,
            },
            // Draw only happens if auto-rematch exhausted all rounds (extremely rare)
            Outcome::Draw => Self {
                outcome: *outcome,
                captured_amount: price,
                refund_amount: Decimal::ZERO,
                reward_token: None,
                reward_amount: 0,
            },
            Outcome::Lose => Self {
                outcome: *outcome,
                captured_amount: price,
                refund_amount: Decimal::ZERO,
                reward_token: None,
                reward_amount: 0,
            },
        }
    }
}

/// Executes the full settlement inside a transaction.
/// The game must already be in a RESOLVED_* state.
pub async fn execute(pool: &PgPool, game: &GameRow) -> Result<Uuid, AppError> {
    let outcome = game
        .result
        .ok_or_else(|| AppError::Internal(anyhow::anyhow!("game has no result")))?;

    let plan = SettlementPlan::from_outcome(&outcome, game.price, &game.user_choice);

    let mut tx = pool.begin().await?;

    // Transition to SETTLING
    let settling_status = GameStatus::Settling;
    if !game.status.can_transition_to(&settling_status) {
        return Err(AppError::Conflict(format!(
            "cannot transition from {:?} to settling",
            game.status
        )));
    }
    db::games::transition(&mut *tx, game.id, GameStatus::Settling).await?;

    // Create settlement record
    let settlement = db::settlements::create(
        &mut *tx,
        db::settlements::CreateSettlementParams {
            game_id: game.id,
            outcome: plan.outcome,
            refund_amount: plan.refund_amount,
            captured_amount: plan.captured_amount,
            reward_token: plan.reward_token,
            reward_amount: plan.reward_amount,
        },
    )
    .await?;

    // Credit inventory on win
    if let (Some(token), Some(user_id)) = (plan.reward_token, game.user_id) {
        if plan.reward_amount > 0 {
            db::inventories::credit(&mut *tx, user_id, token, plan.reward_amount).await?;
        }
    }

    // Mark game as settled
    db::games::mark_settled(&mut *tx, game.id).await?;

    tx.commit().await?;

    Ok(settlement.id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_settlement_plan_win() {
        let plan =
            SettlementPlan::from_outcome(&Outcome::Win, Decimal::new(5, 2), &Choice::Rock);
        assert_eq!(plan.captured_amount, Decimal::new(5, 2));
        assert_eq!(plan.refund_amount, Decimal::ZERO);
        assert_eq!(plan.reward_token, Some(TokenType::Rock));
        assert_eq!(plan.reward_amount, 1);
    }

    #[test]
    fn test_settlement_plan_draw_captures_full() {
        let plan =
            SettlementPlan::from_outcome(&Outcome::Draw, Decimal::new(5, 2), &Choice::Paper);
        assert_eq!(plan.captured_amount, Decimal::new(5, 2));
        assert_eq!(plan.refund_amount, Decimal::ZERO);
        assert!(plan.reward_token.is_none());
        assert_eq!(plan.reward_amount, 0);
    }

    #[test]
    fn test_settlement_plan_lose() {
        let plan = SettlementPlan::from_outcome(
            &Outcome::Lose,
            Decimal::new(5, 2),
            &Choice::Scissors,
        );
        assert_eq!(plan.captured_amount, Decimal::new(5, 2));
        assert_eq!(plan.refund_amount, Decimal::ZERO);
        assert!(plan.reward_token.is_none());
        assert_eq!(plan.reward_amount, 0);
    }
}
