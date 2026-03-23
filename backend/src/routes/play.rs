use std::sync::Arc;

use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::post;
use axum::{Json, Router};
use mpp::{format_receipt, format_www_authenticate};
use mpp::server::axum::ChallengeOptions;
use serde_json::json;

use crate::app::AppState;
use crate::db;
use crate::domain::{fairness, game, payer, settlement};
use crate::error::AppError;
use crate::types::api::{PlayRequest, PlayResultResponse, RoundSummary, SettlementSummary};
use crate::types::domain::{GameStatus, Outcome};

pub fn router() -> Router<Arc<AppState>> {
    Router::new().route("/play", post(handle_play))
}

async fn handle_play(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(req): Json<PlayRequest>,
) -> Result<Response, AppError> {
    let auth_header = headers
        .get("authorization")
        .and_then(|v| v.to_str().ok());

    match auth_header {
        None => handle_402_challenge(state, req).await,
        Some(auth) => handle_paid_play(state, req, auth).await,
    }
}

async fn handle_402_challenge(
    state: Arc<AppState>,
    req: PlayRequest,
) -> Result<Response, AppError> {
    let server_choice = game::random_choice();
    let salt = fairness::generate_salt();

    let game_id = uuid::Uuid::new_v4();
    let commit = fairness::compute_commit(&game_id, &server_choice, &salt);

    let game_row = db::games::create(
        &state.db,
        db::games::CreateGameParams {
            id: Some(game_id),
            price: state.config.play_price,
            currency: state.config.play_currency.clone(),
            user_choice: req.choice,
            server_choice,
            server_salt: salt,
            server_commit: commit.clone(),
            rounds: serde_json::Value::Array(vec![]),
        },
    )
    .await?;

    tracing::info!(game_id = %game_row.id, "game created, returning 402");

    let challenge = state
        .mpp
        .challenge(&state.config.play_price_str(), ChallengeOptions::default())
        .map_err(|e| AppError::Internal(anyhow::anyhow!("mpp charge error: {e}")))?;

    let www_auth = format_www_authenticate(&challenge)
        .map_err(|e| AppError::Internal(anyhow::anyhow!("mpp format error: {e}")))?;

    let body = json!({
        "error": "payment_required",
        "game_id": game_row.id,
        "amount": state.config.play_price_str(),
        "currency": state.config.play_currency,
        "server_commit": commit,
        "expires_at": (game_row.created_at + chrono::Duration::seconds(state.config.game_ttl_seconds as i64)).to_rfc3339(),
    });

    Ok((
        StatusCode::PAYMENT_REQUIRED,
        [
            ("www-authenticate", www_auth),
            ("cache-control", "no-store".to_string()),
        ],
        Json(body),
    )
        .into_response())
}

/// Authorization header present: verify payment, resolve game with auto-rematch, settle.
async fn handle_paid_play(
    state: Arc<AppState>,
    req: PlayRequest,
    auth_header: &str,
) -> Result<Response, AppError> {
    let receipt = state
        .mpp
        .verify_payment(auth_header)
        .await
        .map_err(|e| AppError::PaymentInvalid(format!("payment verification failed: {e}")))?;

    let payer_wallet =
        payer::resolve_payer_wallet(auth_header, &receipt.reference, &state.tempo_provider)
            .await?;
    let user = db::users::upsert_by_wallet(&state.db, &payer_wallet).await?;

    let mut tx = state.db.begin().await?;

    let game_record = if let Some(game_id) = req.game_id {
        let g = db::games::lock_for_update(&mut *tx, game_id, GameStatus::PaymentRequired)
            .await?
            .ok_or_else(|| {
                AppError::NotFound(format!(
                    "game {game_id} not found or not in payment_required state"
                ))
            })?;

        let ttl = chrono::Duration::seconds(state.config.game_ttl_seconds as i64);
        if g.created_at + ttl < chrono::Utc::now() {
            db::games::transition(&mut *tx, game_id, GameStatus::Expired).await?;
            tx.commit().await?;
            return Err(AppError::Gone(format!("game {game_id} has expired")));
        }

        if g.user_choice != req.choice {
            return Err(AppError::Validation(
                "choice does not match original game request".to_string(),
            ));
        }

        g
    } else {
        let server_choice = game::random_choice();
        let salt = fairness::generate_salt();
        let new_game_id = uuid::Uuid::new_v4();
        let commit = fairness::compute_commit(&new_game_id, &server_choice, &salt);

        db::games::create_with_tx(
            &mut *tx,
            db::games::CreateGameParams {
                id: Some(new_game_id),
                price: state.config.play_price,
                currency: state.config.play_currency.clone(),
                user_choice: req.choice,
                server_choice,
                server_salt: salt,
                server_commit: commit,
                rounds: serde_json::Value::Array(vec![]),
            },
        )
        .await?
    };

    let game_id = game_record.id;

    db::games::set_user_id(&mut *tx, game_id, user.id).await?;
    db::games::transition(&mut *tx, game_id, GameStatus::PaymentAuthorized).await?;

    db::payments::create(
        &mut *tx,
        db::payments::CreatePaymentParams {
            game_id,
            amount: game_record.price,
            provider_payment_id: Some(receipt.reference.clone()),
            authorization_payload: None,
            receipt_payload: None,
        },
    )
    .await?;

    db::games::transition(&mut *tx, game_id, GameStatus::PlayLocked).await?;

    // --- Auto-rematch resolution ---
    // Round 1: use the pre-committed choice from the game record
    let mut rounds = Vec::new();
    let round1_result = game::resolve(&game_record.user_choice, &game_record.server_choice);
    rounds.push(game::RoundResult {
        round: 1,
        server_choice: game_record.server_choice,
        server_salt: game_record.server_salt.clone(),
        server_commit: game_record.server_commit.clone(),
        user_choice: game_record.user_choice,
        result: round1_result,
    });

    let mut final_outcome = round1_result;

    // If draw, auto-rematch with new random choices
    if final_outcome == Outcome::Draw {
        for round_num in 2..=game::MAX_REMATCH_ROUNDS {
            let sc = game::random_choice();
            let salt = fairness::generate_salt();
            let commit = fairness::compute_commit(&game_id, &sc, &salt);
            let result = game::resolve(&game_record.user_choice, &sc);

            rounds.push(game::RoundResult {
                round: round_num,
                server_choice: sc,
                server_salt: salt,
                server_commit: commit,
                user_choice: game_record.user_choice,
                result,
            });

            final_outcome = result;
            if result != Outcome::Draw {
                break;
            }
        }
    }

    let final_round = rounds.last().unwrap();
    let rounds_json = serde_json::to_value(&rounds)
        .map_err(|e| AppError::Internal(anyhow::anyhow!("serialize rounds: {e}")))?;

    let resolved_status = GameStatus::for_outcome(&final_outcome);
    db::games::resolve_with_rounds(
        &mut *tx,
        game_id,
        final_outcome,
        resolved_status,
        &final_round.server_choice,
        &final_round.server_salt,
        &final_round.server_commit,
        &rounds_json,
    )
    .await?;

    let plan = settlement::SettlementPlan::from_outcome(
        &final_outcome,
        game_record.price,
        &game_record.user_choice,
    );

    db::games::transition(&mut *tx, game_id, GameStatus::Settling).await?;

    let settlement_row = db::settlements::create(
        &mut *tx,
        db::settlements::CreateSettlementParams {
            game_id,
            outcome: plan.outcome,
            refund_amount: plan.refund_amount,
            captured_amount: plan.captured_amount,
            reward_token: plan.reward_token,
            reward_amount: plan.reward_amount,
        },
    )
    .await?;

    if let Some(token) = plan.reward_token {
        if plan.reward_amount > 0 {
            db::inventories::credit(&mut *tx, user.id, token, plan.reward_amount).await?;
        }
    }

    db::games::mark_settled(&mut *tx, game_id).await?;

    tx.commit().await?;

    tracing::info!(
        game_id = %game_id,
        outcome = ?final_outcome,
        total_rounds = rounds.len(),
        user = %payer_wallet,
        "game resolved and settled"
    );

    let round_summaries: Vec<RoundSummary> = rounds
        .iter()
        .map(|r| RoundSummary {
            round: r.round,
            server_choice: r.server_choice,
            server_salt: r.server_salt.clone(),
            server_commit: r.server_commit.clone(),
            user_choice: r.user_choice,
            result: r.result,
        })
        .collect();

    let total_rounds = round_summaries.len();
    let response = PlayResultResponse {
        game_id,
        result: final_outcome,
        user_choice: game_record.user_choice,
        server_choice: final_round.server_choice,
        server_salt: final_round.server_salt.clone(),
        server_commit: final_round.server_commit.clone(),
        rounds: round_summaries,
        total_rounds,
        settlement: SettlementSummary {
            reward_token: plan.reward_token,
            reward_amount: plan.reward_amount,
            captured_amount: plan.captured_amount.to_string(),
        },
        receipt_id: settlement_row.id,
    };

    let receipt_header = format_receipt(&receipt)
        .unwrap_or_else(|_| receipt.reference.clone());

    Ok((
        StatusCode::OK,
        [("payment-receipt", receipt_header)],
        Json(response),
    )
        .into_response())
}
