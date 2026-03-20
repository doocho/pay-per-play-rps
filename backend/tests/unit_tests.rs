use pay_per_play_rps::types::domain::*;
use pay_per_play_rps::domain::fairness;
use pay_per_play_rps::domain::game;
use pay_per_play_rps::domain::settlement::SettlementPlan;
use rust_decimal::Decimal;
use uuid::Uuid;

// --- Game resolution: all 9 outcomes ---

#[test]
fn rock_vs_rock_is_draw() {
    assert_eq!(game::resolve(&Choice::Rock, &Choice::Rock), Outcome::Draw);
}

#[test]
fn rock_vs_scissors_is_win() {
    assert_eq!(game::resolve(&Choice::Rock, &Choice::Scissors), Outcome::Win);
}

#[test]
fn rock_vs_paper_is_lose() {
    assert_eq!(game::resolve(&Choice::Rock, &Choice::Paper), Outcome::Lose);
}

#[test]
fn paper_vs_paper_is_draw() {
    assert_eq!(game::resolve(&Choice::Paper, &Choice::Paper), Outcome::Draw);
}

#[test]
fn paper_vs_rock_is_win() {
    assert_eq!(game::resolve(&Choice::Paper, &Choice::Rock), Outcome::Win);
}

#[test]
fn paper_vs_scissors_is_lose() {
    assert_eq!(game::resolve(&Choice::Paper, &Choice::Scissors), Outcome::Lose);
}

#[test]
fn scissors_vs_scissors_is_draw() {
    assert_eq!(game::resolve(&Choice::Scissors, &Choice::Scissors), Outcome::Draw);
}

#[test]
fn scissors_vs_paper_is_win() {
    assert_eq!(game::resolve(&Choice::Scissors, &Choice::Paper), Outcome::Win);
}

#[test]
fn scissors_vs_rock_is_lose() {
    assert_eq!(game::resolve(&Choice::Scissors, &Choice::Rock), Outcome::Lose);
}

// --- Fairness commit-reveal ---

#[test]
fn fairness_commit_roundtrip() {
    let game_id = Uuid::new_v4();
    let choice = Choice::Paper;
    let salt = fairness::generate_salt();
    let commit = fairness::compute_commit(&game_id, &choice, &salt);

    assert!(fairness::verify_commit(&game_id, &choice, &salt, &commit));
}

#[test]
fn fairness_commit_different_choice_fails() {
    let game_id = Uuid::new_v4();
    let salt = fairness::generate_salt();
    let commit = fairness::compute_commit(&game_id, &Choice::Rock, &salt);

    assert!(!fairness::verify_commit(&game_id, &Choice::Paper, &salt, &commit));
}

#[test]
fn fairness_commit_different_salt_fails() {
    let game_id = Uuid::new_v4();
    let choice = Choice::Scissors;
    let salt = fairness::generate_salt();
    let commit = fairness::compute_commit(&game_id, &choice, &salt);

    assert!(!fairness::verify_commit(&game_id, &choice, "tampered-salt", &commit));
}

#[test]
fn fairness_commit_different_game_id_fails() {
    let game_id = Uuid::new_v4();
    let other_id = Uuid::new_v4();
    let choice = Choice::Rock;
    let salt = fairness::generate_salt();
    let commit = fairness::compute_commit(&game_id, &choice, &salt);

    assert!(!fairness::verify_commit(&other_id, &choice, &salt, &commit));
}

// --- Settlement plan ---

#[test]
fn settlement_win_captures_full_and_rewards() {
    let price = Decimal::new(5, 2);
    let plan = SettlementPlan::from_outcome(&Outcome::Win, price, &Choice::Rock);

    assert_eq!(plan.captured_amount, price);
    assert_eq!(plan.refund_amount, Decimal::ZERO);
    assert_eq!(plan.reward_token, Some(TokenType::Rock));
    assert_eq!(plan.reward_amount, 1);
}

#[test]
fn settlement_draw_captures_full_no_reward() {
    let price = Decimal::new(5, 2);
    let plan = SettlementPlan::from_outcome(&Outcome::Draw, price, &Choice::Paper);

    assert_eq!(plan.captured_amount, price);
    assert_eq!(plan.refund_amount, Decimal::ZERO);
    assert!(plan.reward_token.is_none());
    assert_eq!(plan.reward_amount, 0);
}

#[test]
fn settlement_lose_captures_no_reward() {
    let price = Decimal::new(5, 2);
    let plan = SettlementPlan::from_outcome(&Outcome::Lose, price, &Choice::Scissors);

    assert_eq!(plan.captured_amount, price);
    assert_eq!(plan.refund_amount, Decimal::ZERO);
    assert!(plan.reward_token.is_none());
    assert_eq!(plan.reward_amount, 0);
}

#[test]
fn settlement_reward_token_matches_choice() {
    let price = Decimal::new(1, 0);

    let rock = SettlementPlan::from_outcome(&Outcome::Win, price, &Choice::Rock);
    assert_eq!(rock.reward_token, Some(TokenType::Rock));

    let paper = SettlementPlan::from_outcome(&Outcome::Win, price, &Choice::Paper);
    assert_eq!(paper.reward_token, Some(TokenType::Paper));

    let scissors = SettlementPlan::from_outcome(&Outcome::Win, price, &Choice::Scissors);
    assert_eq!(scissors.reward_token, Some(TokenType::Scissors));
}

// --- Game state machine transitions ---

#[test]
fn valid_transitions() {
    assert!(GameStatus::Created.can_transition_to(&GameStatus::PaymentRequired));
    assert!(GameStatus::PaymentRequired.can_transition_to(&GameStatus::PaymentAuthorized));
    assert!(GameStatus::PaymentRequired.can_transition_to(&GameStatus::Expired));
    assert!(GameStatus::PaymentAuthorized.can_transition_to(&GameStatus::PlayLocked));
    assert!(GameStatus::PlayLocked.can_transition_to(&GameStatus::ResolvedWin));
    assert!(GameStatus::PlayLocked.can_transition_to(&GameStatus::ResolvedDraw));
    assert!(GameStatus::PlayLocked.can_transition_to(&GameStatus::ResolvedLose));
    assert!(GameStatus::ResolvedWin.can_transition_to(&GameStatus::Settling));
    assert!(GameStatus::ResolvedDraw.can_transition_to(&GameStatus::Settling));
    assert!(GameStatus::ResolvedLose.can_transition_to(&GameStatus::Settling));
    assert!(GameStatus::Settling.can_transition_to(&GameStatus::Settled));
}

#[test]
fn invalid_transitions() {
    assert!(!GameStatus::Created.can_transition_to(&GameStatus::PlayLocked));
    assert!(!GameStatus::PaymentRequired.can_transition_to(&GameStatus::Settled));
    assert!(!GameStatus::Settled.can_transition_to(&GameStatus::Created));
    assert!(!GameStatus::Expired.can_transition_to(&GameStatus::PaymentAuthorized));
    assert!(!GameStatus::ResolvedWin.can_transition_to(&GameStatus::PlayLocked));
    assert!(!GameStatus::PlayLocked.can_transition_to(&GameStatus::PaymentRequired));
}

#[test]
fn status_for_outcome() {
    assert_eq!(GameStatus::for_outcome(&Outcome::Win), GameStatus::ResolvedWin);
    assert_eq!(GameStatus::for_outcome(&Outcome::Draw), GameStatus::ResolvedDraw);
    assert_eq!(GameStatus::for_outcome(&Outcome::Lose), GameStatus::ResolvedLose);
}

// --- Auto-rematch ---

#[test]
fn resolve_with_rematch_returns_at_least_one_round() {
    let game_id = Uuid::new_v4();
    let (outcome, rounds) = game::resolve_with_rematch(&Choice::Rock, &game_id, 10);

    assert!(!rounds.is_empty());
    assert!(rounds.len() <= 10);
    assert_eq!(rounds.last().unwrap().result, outcome);
}

#[test]
fn resolve_with_rematch_all_intermediate_rounds_are_draws() {
    let game_id = Uuid::new_v4();
    let (_outcome, rounds) = game::resolve_with_rematch(&Choice::Paper, &game_id, 10);

    for r in &rounds[..rounds.len() - 1] {
        assert_eq!(r.result, Outcome::Draw);
    }
}

#[test]
fn resolve_with_rematch_round_numbers_are_sequential() {
    let game_id = Uuid::new_v4();
    let (_outcome, rounds) = game::resolve_with_rematch(&Choice::Scissors, &game_id, 10);

    for (i, r) in rounds.iter().enumerate() {
        assert_eq!(r.round, i + 1);
    }
}

#[test]
fn resolve_with_rematch_user_choice_preserved() {
    let game_id = Uuid::new_v4();
    let (_outcome, rounds) = game::resolve_with_rematch(&Choice::Rock, &game_id, 10);

    for r in &rounds {
        assert_eq!(r.user_choice, Choice::Rock);
    }
}

#[test]
fn resolve_with_rematch_max_1_returns_single_round() {
    let game_id = Uuid::new_v4();
    let (_outcome, rounds) = game::resolve_with_rematch(&Choice::Rock, &game_id, 1);

    assert_eq!(rounds.len(), 1);
}

#[test]
fn resolve_with_rematch_each_round_has_valid_commit() {
    let game_id = Uuid::new_v4();
    let (_outcome, rounds) = game::resolve_with_rematch(&Choice::Rock, &game_id, 10);

    for r in &rounds {
        let verified = fairness::verify_commit(&game_id, &r.server_choice, &r.server_salt, &r.server_commit);
        assert!(verified, "round {} commit should verify", r.round);
    }
}
