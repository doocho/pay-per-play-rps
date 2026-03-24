use rand::Rng;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::domain::game;
use crate::types::domain::Choice;
use crate::types::pvp::{PvpGameRow, PvpGameStatus, PvpOutcome};

pub const MAX_REMATCH_ROUNDS: i32 = 10;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PvpRoundResult {
    pub round: i32,
    pub player1_choice: Choice,
    pub player1_commit: String,
    pub player2_choice: Choice,
    pub player2_commit: String,
    pub result: PvpOutcome,
}

/// Generate a short, human-friendly room code (6 alphanumeric chars).
pub fn generate_room_code() -> String {
    const CHARSET: &[u8] = b"ABCDEFGHJKLMNPQRSTUVWXYZ23456789";
    let mut rng = rand::rng();
    (0..6)
        .map(|_| {
            let idx = rng.random_range(0..CHARSET.len());
            CHARSET[idx] as char
        })
        .collect()
}

/// Resolve a PvP round given both players' choices.
pub fn resolve_pvp(player1: &Choice, player2: &Choice) -> PvpOutcome {
    let outcome = game::resolve(player1, player2);
    match outcome {
        crate::types::domain::Outcome::Win => PvpOutcome::Player1Wins,
        crate::types::domain::Outcome::Lose => PvpOutcome::Player2Wins,
        crate::types::domain::Outcome::Draw => PvpOutcome::Draw,
    }
}

/// Determine the new status after a player pays.
pub fn status_after_payment(game: &PvpGameRow, player_number: u8) -> PvpGameStatus {
    match (game.status, player_number) {
        (PvpGameStatus::WaitingForOpponent, 1) => PvpGameStatus::Player1Paid,
        (PvpGameStatus::WaitingForOpponent, 2) => PvpGameStatus::Player2Paid,
        (PvpGameStatus::Player1Paid, 2) => PvpGameStatus::BothPaid,
        (PvpGameStatus::Player2Paid, 1) => PvpGameStatus::BothPaid,
        _ => game.status, // no-op for invalid transitions
    }
}

/// Determine the new status after a player commits.
pub fn status_after_commit(game: &PvpGameRow, player_number: u8) -> Option<PvpGameStatus> {
    match (game.status, player_number) {
        (PvpGameStatus::BothPaid, 1) => Some(PvpGameStatus::Player1Committed),
        (PvpGameStatus::BothPaid, 2) => Some(PvpGameStatus::Player2Committed),
        (PvpGameStatus::Player1Committed, 2) => Some(PvpGameStatus::BothCommitted),
        (PvpGameStatus::Player2Committed, 1) => Some(PvpGameStatus::BothCommitted),
        _ => None,
    }
}

/// Determine the new status after a player reveals.
pub fn status_after_reveal(game: &PvpGameRow, player_number: u8) -> Option<PvpGameStatus> {
    match (game.status, player_number) {
        (PvpGameStatus::BothCommitted, 1) => Some(PvpGameStatus::Player1Revealed),
        (PvpGameStatus::BothCommitted, 2) => Some(PvpGameStatus::Player2Revealed),
        (PvpGameStatus::Player1Revealed, 2) => None, // both revealed → resolve
        (PvpGameStatus::Player2Revealed, 1) => None, // both revealed → resolve
        _ => None,
    }
}

/// Check if both reveals are now complete (second player just revealed).
pub fn both_revealed(game: &PvpGameRow, player_number: u8) -> bool {
    matches!(
        (game.status, player_number),
        (PvpGameStatus::Player1Revealed, 2) | (PvpGameStatus::Player2Revealed, 1)
    )
}

/// Determine the resolved game status from outcome.
pub fn status_for_outcome(outcome: &PvpOutcome) -> PvpGameStatus {
    match outcome {
        PvpOutcome::Player1Wins => PvpGameStatus::ResolvedPlayer1Wins,
        PvpOutcome::Player2Wins => PvpGameStatus::ResolvedPlayer2Wins,
        PvpOutcome::Draw => PvpGameStatus::ResolvedDraw,
    }
}

/// Determine who won based on outcome and return winner's user_id.
pub fn winner_id(game: &PvpGameRow, outcome: &PvpOutcome) -> Option<Uuid> {
    match outcome {
        PvpOutcome::Player1Wins => game.player1_id,
        PvpOutcome::Player2Wins => game.player2_id,
        PvpOutcome::Draw => None,
    }
}

/// Get the winner's choice for token reward.
pub fn winner_choice(game: &PvpGameRow, outcome: &PvpOutcome) -> Option<Choice> {
    match outcome {
        PvpOutcome::Player1Wins => game.player1_choice,
        PvpOutcome::Player2Wins => game.player2_choice,
        PvpOutcome::Draw => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolve_pvp() {
        assert_eq!(
            resolve_pvp(&Choice::Rock, &Choice::Scissors),
            PvpOutcome::Player1Wins
        );
        assert_eq!(
            resolve_pvp(&Choice::Rock, &Choice::Paper),
            PvpOutcome::Player2Wins
        );
        assert_eq!(
            resolve_pvp(&Choice::Rock, &Choice::Rock),
            PvpOutcome::Draw
        );
    }

    #[test]
    fn test_room_code_format() {
        let code = generate_room_code();
        assert_eq!(code.len(), 6);
        assert!(code.chars().all(|c| c.is_ascii_alphanumeric()));
    }
}
