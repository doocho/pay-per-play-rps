use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::domain::{Choice, TokenType};
use super::pvp::PvpOutcome;

// --- Create room ---

#[derive(Debug, Serialize)]
pub struct PvpCreateResponse {
    pub game_id: Uuid,
    pub room_code: String,
    pub price: String,
    pub currency: String,
}

// --- Join room ---

#[derive(Debug, Serialize)]
pub struct PvpJoinResponse {
    pub game_id: Uuid,
    pub status: String,
}

// --- Queue ---

#[derive(Debug, Serialize)]
pub struct PvpQueueResponse {
    pub game_id: Uuid,
    pub status: String,
    /// If matched immediately, this will be true.
    pub matched: bool,
}

// --- Commit ---

#[derive(Debug, Deserialize)]
pub struct PvpCommitRequest {
    pub commit: String,
}

#[derive(Debug, Serialize)]
pub struct PvpCommitResponse {
    pub game_id: Uuid,
    pub status: String,
    pub round: i32,
}

// --- Reveal ---

#[derive(Debug, Deserialize)]
pub struct PvpRevealRequest {
    pub choice: Choice,
    pub salt: String,
}

#[derive(Debug, Serialize)]
pub struct PvpRevealResponse {
    pub game_id: Uuid,
    pub status: String,
    /// Only present when both reveals are in and game is resolved.
    pub result: Option<PvpResultDetail>,
}

#[derive(Debug, Serialize)]
pub struct PvpResultDetail {
    pub outcome: PvpOutcome,
    pub your_choice: Choice,
    pub opponent_choice: Choice,
    pub settlement: PvpSettlementSummary,
    pub receipt_id: Uuid,
}

#[derive(Debug, Serialize)]
pub struct PvpSettlementSummary {
    pub pot_amount: String,
    pub platform_fee: String,
    pub winner_payout: String,
    pub reward_token: Option<TokenType>,
    pub reward_amount: i32,
}

// --- Game detail (polling) ---

#[derive(Debug, Serialize)]
pub struct PvpGameDetailResponse {
    pub id: Uuid,
    pub room_code: Option<String>,
    pub status: String,
    pub price: String,
    pub currency: String,
    pub current_round: i32,
    /// Which player you are (1 or 2). None if not a participant.
    pub your_player_number: Option<u8>,
    /// Your choice (only after reveal)
    pub your_choice: Option<Choice>,
    /// Opponent's choice (only after game resolved)
    pub opponent_choice: Option<Choice>,
    pub result: Option<PvpOutcome>,
    pub settlement: Option<PvpSettlementSummary>,
    pub created_at: String,
    pub resolved_at: Option<String>,
    pub settled_at: Option<String>,
}

// --- Fairness ---

#[derive(Debug, Serialize)]
pub struct PvpFairnessResponse {
    pub game_id: Uuid,
    pub player1_commit: Option<String>,
    pub player1_choice: Option<Choice>,
    pub player1_salt: Option<String>,
    pub player1_verified: Option<bool>,
    pub player2_commit: Option<String>,
    pub player2_choice: Option<Choice>,
    pub player2_salt: Option<String>,
    pub player2_verified: Option<bool>,
}
