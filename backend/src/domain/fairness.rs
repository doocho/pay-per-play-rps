use hex;
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::types::domain::Choice;

pub fn generate_salt() -> String {
    let mut bytes = [0u8; 32];
    rand::fill(&mut bytes);
    hex::encode(bytes)
}

pub fn compute_commit(game_id: &Uuid, choice: &Choice, salt: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(game_id.to_string().as_bytes());
    hasher.update(b"||");
    hasher.update(choice.to_string().as_bytes());
    hasher.update(b"||");
    hasher.update(salt.as_bytes());
    hex::encode(hasher.finalize())
}

pub fn verify_commit(
    game_id: &Uuid,
    choice: &Choice,
    salt: &str,
    expected_commit: &str,
) -> bool {
    let recomputed = compute_commit(game_id, choice, salt);
    recomputed == expected_commit
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_commit_verify_roundtrip() {
        let game_id = Uuid::new_v4();
        let choice = Choice::Rock;
        let salt = generate_salt();
        let commit = compute_commit(&game_id, &choice, &salt);

        assert!(verify_commit(&game_id, &choice, &salt, &commit));
        assert!(!verify_commit(&game_id, &Choice::Paper, &salt, &commit));
        assert!(!verify_commit(&game_id, &choice, "wrong-salt", &commit));
    }
}
