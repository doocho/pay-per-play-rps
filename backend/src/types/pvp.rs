use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

use super::domain::{Choice, TokenType};

// --- Enums ---

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "pvp_game_status", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum PvpGameStatus {
    WaitingForOpponent,
    Player1Paid,
    Player2Paid,
    BothPaid,
    Player1Committed,
    Player2Committed,
    BothCommitted,
    Player1Revealed,
    Player2Revealed,
    ResolvedPlayer1Wins,
    ResolvedPlayer2Wins,
    ResolvedDraw,
    Settling,
    Settled,
    Expired,
    Cancelled,
}

impl PvpGameStatus {
    pub fn is_waiting_payment(&self) -> bool {
        matches!(
            self,
            Self::WaitingForOpponent | Self::Player1Paid | Self::Player2Paid
        )
    }

    pub fn is_waiting_commit(&self) -> bool {
        matches!(
            self,
            Self::BothPaid | Self::Player1Committed | Self::Player2Committed
        )
    }

    pub fn is_waiting_reveal(&self) -> bool {
        matches!(
            self,
            Self::BothCommitted | Self::Player1Revealed | Self::Player2Revealed
        )
    }

    pub fn is_resolved(&self) -> bool {
        matches!(
            self,
            Self::ResolvedPlayer1Wins | Self::ResolvedPlayer2Wins | Self::ResolvedDraw
        )
    }

    pub fn is_terminal(&self) -> bool {
        matches!(self, Self::Settled | Self::Expired | Self::Cancelled)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "pvp_outcome", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum PvpOutcome {
    Player1Wins,
    Player2Wins,
    Draw,
}

// --- Row types ---

#[derive(Debug, Clone, FromRow, Serialize)]
pub struct PvpGameRow {
    pub id: Uuid,
    pub room_code: Option<String>,
    pub player1_id: Option<Uuid>,
    pub player2_id: Option<Uuid>,
    pub status: PvpGameStatus,
    pub price: Decimal,
    pub currency: String,
    pub platform_fee_bps: i32,
    pub player1_choice: Option<Choice>,
    pub player1_salt: Option<String>,
    pub player1_commit: Option<String>,
    pub player2_choice: Option<Choice>,
    pub player2_salt: Option<String>,
    pub player2_commit: Option<String>,
    pub result: Option<PvpOutcome>,
    pub rounds: serde_json::Value,
    pub current_round: i32,
    pub created_at: DateTime<Utc>,
    pub player2_joined_at: Option<DateTime<Utc>>,
    pub both_paid_at: Option<DateTime<Utc>>,
    pub both_committed_at: Option<DateTime<Utc>>,
    pub resolved_at: Option<DateTime<Utc>>,
    pub settled_at: Option<DateTime<Utc>>,
}

impl PvpGameRow {
    /// Determine which player number (1 or 2) a given user_id is.
    pub fn player_number(&self, user_id: Uuid) -> Option<u8> {
        if self.player1_id == Some(user_id) {
            Some(1)
        } else if self.player2_id == Some(user_id) {
            Some(2)
        } else {
            None
        }
    }
}

#[derive(Debug, Clone, FromRow, Serialize)]
pub struct PvpPaymentRow {
    pub id: Uuid,
    pub pvp_game_id: Uuid,
    pub player_id: Uuid,
    pub amount: Decimal,
    pub protocol: String,
    pub network: String,
    pub asset: String,
    pub provider_payment_id: Option<String>,
    pub authorization_payload: Option<serde_json::Value>,
    pub receipt_payload: Option<serde_json::Value>,
    pub status: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, FromRow, Serialize)]
pub struct PvpSettlementRow {
    pub id: Uuid,
    pub pvp_game_id: Uuid,
    pub result: PvpOutcome,
    pub pot_amount: Decimal,
    pub platform_fee: Decimal,
    pub winner_payout: Decimal,
    pub loser_refund: Decimal,
    pub winner_id: Option<Uuid>,
    pub reward_token: Option<TokenType>,
    pub reward_amount: i32,
    pub status: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, FromRow, Serialize)]
pub struct MatchmakingQueueRow {
    pub id: Uuid,
    pub user_id: Uuid,
    pub pvp_game_id: Uuid,
    pub price: Decimal,
    pub currency: String,
    pub enqueued_at: DateTime<Utc>,
}
