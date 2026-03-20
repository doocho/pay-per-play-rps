use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::domain::fairness;
use crate::types::domain::{Choice, Outcome};
use rand::Rng;

pub const MAX_REMATCH_ROUNDS: usize = 10;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoundResult {
    pub round: usize,
    pub server_choice: Choice,
    pub server_salt: String,
    pub server_commit: String,
    pub user_choice: Choice,
    pub result: Outcome,
}

pub fn resolve(user: &Choice, server: &Choice) -> Outcome {
    if user == server {
        return Outcome::Draw;
    }
    match (user, server) {
        (Choice::Rock, Choice::Scissors)
        | (Choice::Paper, Choice::Rock)
        | (Choice::Scissors, Choice::Paper) => Outcome::Win,
        _ => Outcome::Lose,
    }
}

/// Resolve with auto-rematch on draw. Re-rolls until win/lose or max rounds.
/// Returns the final outcome and all rounds played.
pub fn resolve_with_rematch(
    user_choice: &Choice,
    game_id: &Uuid,
    max_rounds: usize,
) -> (Outcome, Vec<RoundResult>) {
    let mut rounds = Vec::new();

    for round_num in 1..=max_rounds {
        let server_choice = random_choice();
        let salt = fairness::generate_salt();
        let commit = fairness::compute_commit(game_id, &server_choice, &salt);
        let result = resolve(user_choice, &server_choice);

        rounds.push(RoundResult {
            round: round_num,
            server_choice,
            server_salt: salt,
            server_commit: commit,
            user_choice: *user_choice,
            result,
        });

        if result != Outcome::Draw {
            return (result, rounds);
        }
    }

    // All rounds were draws
    (Outcome::Draw, rounds)
}

pub fn random_choice() -> Choice {
    let mut rng = rand::rng();
    match rng.random_range(0u8..3) {
        0 => Choice::Rock,
        1 => Choice::Paper,
        _ => Choice::Scissors,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_all_outcomes() {
        assert_eq!(resolve(&Choice::Rock, &Choice::Rock), Outcome::Draw);
        assert_eq!(resolve(&Choice::Paper, &Choice::Paper), Outcome::Draw);
        assert_eq!(resolve(&Choice::Scissors, &Choice::Scissors), Outcome::Draw);

        assert_eq!(resolve(&Choice::Rock, &Choice::Scissors), Outcome::Win);
        assert_eq!(resolve(&Choice::Paper, &Choice::Rock), Outcome::Win);
        assert_eq!(resolve(&Choice::Scissors, &Choice::Paper), Outcome::Win);

        assert_eq!(resolve(&Choice::Rock, &Choice::Paper), Outcome::Lose);
        assert_eq!(resolve(&Choice::Paper, &Choice::Scissors), Outcome::Lose);
        assert_eq!(resolve(&Choice::Scissors, &Choice::Rock), Outcome::Lose);
    }

    #[test]
    fn resolve_with_rematch_terminates() {
        let game_id = Uuid::new_v4();
        let (outcome, rounds) = resolve_with_rematch(&Choice::Rock, &game_id, MAX_REMATCH_ROUNDS);

        assert!(!rounds.is_empty());
        assert!(rounds.len() <= MAX_REMATCH_ROUNDS);

        let last = rounds.last().unwrap();
        assert_eq!(last.result, outcome);

        // All non-final rounds must be draws
        for r in &rounds[..rounds.len() - 1] {
            assert_eq!(r.result, Outcome::Draw);
        }
    }
}
