use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

// --- Enums ---

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "game_status", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum GameStatus {
    Created,
    PaymentRequired,
    PaymentAuthorized,
    PlayLocked,
    ResolvedWin,
    ResolvedDraw,
    ResolvedLose,
    Settling,
    Settled,
    Expired,
    Failed,
}

impl GameStatus {
    pub fn can_transition_to(&self, target: &GameStatus) -> bool {
        matches!(
            (self, target),
            (Self::Created, Self::PaymentRequired)
                | (Self::PaymentRequired, Self::PaymentAuthorized)
                | (Self::PaymentRequired, Self::Expired)
                | (Self::PaymentAuthorized, Self::PlayLocked)
                | (Self::PlayLocked, Self::ResolvedWin)
                | (Self::PlayLocked, Self::ResolvedDraw)
                | (Self::PlayLocked, Self::ResolvedLose)
                | (Self::ResolvedWin, Self::Settling)
                | (Self::ResolvedDraw, Self::Settling)
                | (Self::ResolvedLose, Self::Settling)
                | (Self::Settling, Self::Settled)
        )
    }

    pub fn for_outcome(outcome: &Outcome) -> Self {
        match outcome {
            Outcome::Win => Self::ResolvedWin,
            Outcome::Draw => Self::ResolvedDraw,
            Outcome::Lose => Self::ResolvedLose,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "choice", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum Choice {
    Rock,
    Paper,
    Scissors,
}

impl std::fmt::Display for Choice {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Choice::Rock => write!(f, "rock"),
            Choice::Paper => write!(f, "paper"),
            Choice::Scissors => write!(f, "scissors"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "outcome", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum Outcome {
    Win,
    Draw,
    Lose,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "token_type", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum TokenType {
    Rock,
    Paper,
    Scissors,
}

impl From<Choice> for TokenType {
    fn from(c: Choice) -> Self {
        match c {
            Choice::Rock => TokenType::Rock,
            Choice::Paper => TokenType::Paper,
            Choice::Scissors => TokenType::Scissors,
        }
    }
}

// --- Row types ---

#[derive(Debug, Clone, FromRow, Serialize)]
pub struct UserRow {
    pub id: Uuid,
    pub wallet_address: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, FromRow, Serialize)]
pub struct GameRow {
    pub id: Uuid,
    pub user_id: Option<Uuid>,
    pub status: GameStatus,
    pub price: Decimal,
    pub currency: String,
    pub user_choice: Choice,
    pub server_choice: Choice,
    pub server_salt: String,
    pub server_commit: String,
    pub result: Option<Outcome>,
    pub rounds: serde_json::Value,
    pub created_at: DateTime<Utc>,
    pub resolved_at: Option<DateTime<Utc>>,
    pub settled_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, FromRow, Serialize)]
pub struct PaymentRow {
    pub id: Uuid,
    pub game_id: Uuid,
    pub protocol: String,
    pub network: String,
    pub asset: String,
    pub amount: Decimal,
    pub status: String,
    pub provider_payment_id: Option<String>,
    pub authorization_payload: Option<serde_json::Value>,
    pub receipt_payload: Option<serde_json::Value>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, FromRow, Serialize)]
pub struct SettlementRow {
    pub id: Uuid,
    pub game_id: Uuid,
    pub outcome: Outcome,
    pub refund_amount: Decimal,
    pub captured_amount: Decimal,
    pub reward_token: Option<TokenType>,
    pub reward_amount: i32,
    pub status: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, FromRow, Serialize)]
pub struct InventoryRow {
    pub id: Uuid,
    pub user_id: Uuid,
    pub token_type: TokenType,
    pub balance: i32,
    pub updated_at: DateTime<Utc>,
}
