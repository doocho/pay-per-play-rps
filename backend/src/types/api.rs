use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::domain::{Choice, Outcome, TokenType};

// --- Play endpoint ---

#[derive(Debug, Deserialize)]
pub struct PlayRequest {
    pub choice: Choice,
    pub game_id: Option<Uuid>,
}

#[derive(Debug, Serialize)]
pub struct PlayResultResponse {
    pub game_id: Uuid,
    pub result: Outcome,
    pub user_choice: Choice,
    pub server_choice: Choice,
    pub server_salt: String,
    pub server_commit: String,
    pub rounds: Vec<RoundSummary>,
    pub total_rounds: usize,
    pub settlement: SettlementSummary,
    pub receipt_id: Uuid,
}

#[derive(Debug, Serialize)]
pub struct RoundSummary {
    pub round: usize,
    pub server_choice: Choice,
    pub server_salt: String,
    pub server_commit: String,
    pub user_choice: Choice,
    pub result: Outcome,
}

#[derive(Debug, Serialize)]
pub struct SettlementSummary {
    pub reward_token: Option<TokenType>,
    pub reward_amount: i32,
    pub captured_amount: String,
}

// --- Game detail ---

#[derive(Debug, Serialize)]
pub struct GameDetailResponse {
    pub id: Uuid,
    pub status: String,
    pub user_choice: Choice,
    pub result: Option<Outcome>,
    pub price: String,
    pub currency: String,
    pub server_choice: Option<Choice>,
    pub server_salt: Option<String>,
    pub server_commit: String,
    pub created_at: String,
    pub resolved_at: Option<String>,
    pub settled_at: Option<String>,
}

// --- Fairness ---

#[derive(Debug, Serialize)]
pub struct FairnessResponse {
    pub game_id: Uuid,
    pub total_rounds: usize,
    pub all_verified: bool,
    pub rounds: Vec<RoundFairnessResult>,
}

#[derive(Debug, Serialize)]
pub struct RoundFairnessResult {
    pub round: usize,
    pub server_choice: Choice,
    pub server_salt: String,
    pub original_commit: String,
    pub recomputed_commit: String,
    pub verified: bool,
}

// --- Receipt ---

#[derive(Debug, Serialize)]
pub struct ReceiptResponse {
    pub receipt_id: Uuid,
    pub game_id: Uuid,
    pub outcome: Outcome,
    pub payment_amount: String,
    pub refund_amount: String,
    pub captured_amount: String,
    pub reward_token: Option<TokenType>,
    pub reward_amount: i32,
    pub settled_at: Option<String>,
}

// --- Leaderboard ---

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct LeaderboardEntry {
    pub wallet_address: String,
    pub total_games: i64,
    pub wins: i64,
    pub draws: i64,
    pub losses: i64,
}

// --- Inventory ---

#[derive(Debug, Serialize)]
pub struct InventoryResponse {
    pub wallet_address: String,
    pub tokens: Vec<TokenBalance>,
}

#[derive(Debug, Serialize)]
pub struct TokenBalance {
    pub token_type: TokenType,
    pub balance: i32,
}

// --- Health ---

#[derive(Debug, Serialize)]
pub struct HealthResponse {
    pub status: String,
    pub db: String,
    pub version: String,
}
