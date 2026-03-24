use std::sync::Arc;

use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{delete, get, post};
use axum::{Json, Router};
use mpp::server::axum::ChallengeOptions;
use mpp::{format_receipt, format_www_authenticate};
use serde_json::json;
use uuid::Uuid;

use crate::app::AppState;
use crate::db;
use crate::domain::{fairness, pvp_game, pvp_settlement};
use crate::error::AppError;
use crate::types::pvp::{PvpGameRow, PvpGameStatus, PvpOutcome};
use crate::types::pvp_api::*;

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/pvp/create", post(handle_create))
        .route("/pvp/join/{room_code}", post(handle_join))
        .route("/pvp/queue", post(handle_queue))
        .route("/pvp/queue", delete(handle_dequeue))
        .route("/pvp/pay/{game_id}", post(handle_pay))
        .route("/pvp/commit/{game_id}", post(handle_commit))
        .route("/pvp/reveal/{game_id}", post(handle_reveal))
        .route("/pvp/game/{game_id}", get(handle_game_detail))
}

// --- Helper: resolve payer wallet and upsert user ---

async fn resolve_user(
    state: &Arc<AppState>,
    auth_header: &str,
) -> Result<crate::types::domain::UserRow, AppError> {
    let receipt = state
        .mpp
        .verify_payment(auth_header)
        .await
        .map_err(|e| AppError::PaymentInvalid(format!("payment verification failed: {e}")))?;

    let payer_wallet =
        crate::domain::payer::resolve_payer_wallet(auth_header, &receipt.reference, &state.tempo_provider)
            .await?;
    let user = db::users::upsert_by_wallet(&state.db, &payer_wallet).await?;
    Ok(user)
}

async fn resolve_user_with_receipt(
    state: &Arc<AppState>,
    auth_header: &str,
) -> Result<(crate::types::domain::UserRow, mpp::Receipt), AppError> {
    let receipt = state
        .mpp
        .verify_payment(auth_header)
        .await
        .map_err(|e| AppError::PaymentInvalid(format!("payment verification failed: {e}")))?;

    let payer_wallet =
        crate::domain::payer::resolve_payer_wallet(auth_header, &receipt.reference, &state.tempo_provider)
            .await?;
    let user = db::users::upsert_by_wallet(&state.db, &payer_wallet).await?;
    Ok((user, receipt))
}

fn make_402_response(
    state: &Arc<AppState>,
    game_id: Uuid,
    created_at: chrono::DateTime<chrono::Utc>,
) -> Result<Response, AppError> {
    let challenge = state
        .mpp
        .challenge(&state.config.pvp_price_str(), ChallengeOptions::default())
        .map_err(|e| AppError::Internal(anyhow::anyhow!("mpp charge error: {e}")))?;

    let www_auth = format_www_authenticate(&challenge)
        .map_err(|e| AppError::Internal(anyhow::anyhow!("mpp format error: {e}")))?;

    let body = json!({
        "error": "payment_required",
        "game_id": game_id,
        "amount": state.config.pvp_price_str(),
        "currency": state.config.pvp_currency.clone(),
        "expires_at": (created_at + chrono::Duration::seconds(state.config.pvp_payment_timeout_seconds as i64)).to_rfc3339(),
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

// ================================
// POST /api/pvp/create
// ================================

async fn handle_create(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> Result<Response, AppError> {
    let auth_header = headers
        .get("authorization")
        .and_then(|v| v.to_str().ok());

    // Step 1: no auth → create game + 402
    let Some(auth) = auth_header else {
        let game_id = Uuid::new_v4();
        let room_code = pvp_game::generate_room_code();

        // We need a user placeholder — game has no user yet, we'll create the game and return 402
        // Actually, we don't know the user until payment. So create with null player1 and set it after payment.
        // But wait, the DB requires player1_id as a FK. Let's adjust: create game after payment.
        // Instead, return the 402 with game_id + room_code in body, create the game once payment is verified.

        // Generate a challenge without creating the game yet
        let challenge = state
            .mpp
            .challenge(&state.config.pvp_price_str(), ChallengeOptions::default())
            .map_err(|e| AppError::Internal(anyhow::anyhow!("mpp charge error: {e}")))?;

        let www_auth = format_www_authenticate(&challenge)
            .map_err(|e| AppError::Internal(anyhow::anyhow!("mpp format error: {e}")))?;

        let body = json!({
            "error": "payment_required",
            "game_id": game_id,
            "room_code": room_code,
            "amount": state.config.pvp_price_str(),
            "currency": state.config.pvp_currency,
        });

        return Ok((
            StatusCode::PAYMENT_REQUIRED,
            [
                ("www-authenticate", www_auth),
                ("cache-control", "no-store".to_string()),
                ("x-pvp-game-id", game_id.to_string()),
                ("x-pvp-room-code", room_code),
            ],
            Json(body),
        )
            .into_response());
    };

    // Step 2: with auth → verify payment, create game
    let (user, receipt) = resolve_user_with_receipt(&state, auth).await?;

    // Extract game_id and room_code from custom headers or body
    // The client should resend with game_id from the 402 response
    let game_id_str = headers
        .get("x-pvp-game-id")
        .and_then(|v| v.to_str().ok())
        .ok_or_else(|| AppError::Validation("missing x-pvp-game-id header".to_string()))?;
    let game_id: Uuid = game_id_str
        .parse()
        .map_err(|_| AppError::Validation("invalid game_id".to_string()))?;

    let room_code = headers
        .get("x-pvp-room-code")
        .and_then(|v| v.to_str().ok())
        .ok_or_else(|| AppError::Validation("missing x-pvp-room-code header".to_string()))?
        .to_string();

    let mut tx = state.db.begin().await?;

    // Check if game already exists (idempotency)
    if let Some(existing) = db::pvp_games::find_by_id(&mut *tx, game_id).await? {
        if existing.player1_id == Some(user.id) {
            tx.commit().await?;
            return Ok(Json(json!({
                "game_id": existing.id,
                "room_code": existing.room_code,
                "status": format!("{:?}", existing.status).to_lowercase(),
            }))
            .into_response());
        }
        return Err(AppError::Conflict("game already exists".to_string()));
    }

    let game = db::pvp_games::create(
        &mut *tx,
        db::pvp_games::CreatePvpGameParams {
            id: game_id,
            room_code: Some(room_code.clone()),
            player1_id: user.id,
            price: state.config.pvp_price,
            currency: state.config.pvp_currency.clone(),
            platform_fee_bps: state.config.pvp_platform_fee_bps,
        },
    )
    .await?;

    // Record payment
    db::pvp_payments::create(
        &mut *tx,
        db::pvp_payments::CreatePvpPaymentParams {
            pvp_game_id: game.id,
            player_id: user.id,
            amount: game.price,
            provider_payment_id: Some(receipt.reference.clone()),
            authorization_payload: None,
            receipt_payload: None,
        },
    )
    .await?;

    // Transition to player1_paid
    db::pvp_games::transition(&mut *tx, game.id, PvpGameStatus::Player1Paid).await?;

    tx.commit().await?;

    tracing::info!(game_id = %game.id, room_code = %room_code, "pvp game created");

    let receipt_header =
        format_receipt(&receipt).unwrap_or_else(|_| receipt.reference.clone());

    Ok((
        StatusCode::CREATED,
        [("payment-receipt", receipt_header)],
        Json(PvpCreateResponse {
            game_id: game.id,
            room_code,
            price: game.price.to_string(),
            currency: game.currency,
        }),
    )
        .into_response())
}

// ================================
// POST /api/pvp/join/{room_code}
// ================================

async fn handle_join(
    State(state): State<Arc<AppState>>,
    Path(room_code): Path<String>,
    headers: HeaderMap,
) -> Result<Response, AppError> {
    let auth_header = headers
        .get("authorization")
        .and_then(|v| v.to_str().ok());

    // Look up game by room code
    let game = db::pvp_games::find_by_room_code(&state.db, &room_code)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("room {room_code} not found")))?;

    if game.player2_id.is_some() {
        return Err(AppError::Conflict("room is full".to_string()));
    }

    if game.status.is_terminal() {
        return Err(AppError::Gone("game is no longer active".to_string()));
    }

    // No auth → 402
    let Some(auth) = auth_header else {
        return make_402_response(&state, game.id, game.created_at);
    };

    // With auth → verify payment, join game
    let (user, receipt) = resolve_user_with_receipt(&state, auth).await?;

    if game.player1_id == Some(user.id) {
        return Err(AppError::Validation(
            "cannot join your own game".to_string(),
        ));
    }

    let mut tx = state.db.begin().await?;

    let game = db::pvp_games::lock_for_update(&mut *tx, game.id)
        .await?
        .ok_or_else(|| AppError::NotFound("game not found".to_string()))?;

    if game.player2_id.is_some() {
        tx.commit().await?;
        return Err(AppError::Conflict("room is full".to_string()));
    }

    let new_status = pvp_game::status_after_payment(&game, 2);

    db::pvp_games::set_player2(&mut *tx, game.id, user.id, new_status).await?;

    // If player1 already paid, we're now both_paid
    if new_status == PvpGameStatus::BothPaid {
        db::pvp_games::set_both_paid(&mut *tx, game.id).await?;
    }

    // Record payment
    db::pvp_payments::create(
        &mut *tx,
        db::pvp_payments::CreatePvpPaymentParams {
            pvp_game_id: game.id,
            player_id: user.id,
            amount: game.price,
            provider_payment_id: Some(receipt.reference.clone()),
            authorization_payload: None,
            receipt_payload: None,
        },
    )
    .await?;

    tx.commit().await?;

    tracing::info!(game_id = %game.id, player2 = %user.wallet_address, "player2 joined");

    let receipt_header =
        format_receipt(&receipt).unwrap_or_else(|_| receipt.reference.clone());

    Ok((
        StatusCode::OK,
        [("payment-receipt", receipt_header)],
        Json(PvpJoinResponse {
            game_id: game.id,
            status: format!("{:?}", new_status).to_lowercase(),
        }),
    )
        .into_response())
}

// ================================
// POST /api/pvp/pay/{game_id}
// ================================

/// For the room creator who creates without paying inline.
/// In our flow, create already handles payment. This endpoint is for
/// the case where player1 needs to pay separately (e.g., matchmaking).
async fn handle_pay(
    State(state): State<Arc<AppState>>,
    Path(game_id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Response, AppError> {
    let game = db::pvp_games::find_by_id(&state.db, game_id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("game {game_id} not found")))?;

    if game.status.is_terminal() {
        return Err(AppError::Gone("game is no longer active".to_string()));
    }

    let auth_header = headers
        .get("authorization")
        .and_then(|v| v.to_str().ok());

    let Some(auth) = auth_header else {
        return make_402_response(&state, game.id, game.created_at);
    };

    let (user, receipt) = resolve_user_with_receipt(&state, auth).await?;

    let player_number = game
        .player_number(user.id)
        .ok_or_else(|| AppError::Validation("you are not a player in this game".to_string()))?;

    // Check if already paid
    let existing_payment =
        db::pvp_payments::find_by_game_and_player(&state.db, game.id, user.id).await?;
    if existing_payment.is_some() {
        return Err(AppError::Conflict("already paid".to_string()));
    }

    let mut tx = state.db.begin().await?;

    let game = db::pvp_games::lock_for_update(&mut *tx, game.id)
        .await?
        .ok_or_else(|| AppError::NotFound("game not found".to_string()))?;

    let new_status = pvp_game::status_after_payment(&game, player_number);

    db::pvp_payments::create(
        &mut *tx,
        db::pvp_payments::CreatePvpPaymentParams {
            pvp_game_id: game.id,
            player_id: user.id,
            amount: game.price,
            provider_payment_id: Some(receipt.reference.clone()),
            authorization_payload: None,
            receipt_payload: None,
        },
    )
    .await?;

    if new_status == PvpGameStatus::BothPaid {
        db::pvp_games::set_both_paid(&mut *tx, game.id).await?;
    } else {
        db::pvp_games::transition(&mut *tx, game.id, new_status).await?;
    }

    tx.commit().await?;

    let receipt_header =
        format_receipt(&receipt).unwrap_or_else(|_| receipt.reference.clone());

    Ok((
        StatusCode::OK,
        [("payment-receipt", receipt_header)],
        Json(json!({
            "game_id": game.id,
            "status": format!("{:?}", new_status).to_lowercase(),
        })),
    )
        .into_response())
}

// ================================
// POST /api/pvp/queue
// ================================

async fn handle_queue(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> Result<Response, AppError> {
    let auth_header = headers
        .get("authorization")
        .and_then(|v| v.to_str().ok());

    // No auth → 402 challenge
    let Some(auth) = auth_header else {
        let challenge = state
            .mpp
            .challenge(&state.config.pvp_price_str(), ChallengeOptions::default())
            .map_err(|e| AppError::Internal(anyhow::anyhow!("mpp charge error: {e}")))?;

        let www_auth = format_www_authenticate(&challenge)
            .map_err(|e| AppError::Internal(anyhow::anyhow!("mpp format error: {e}")))?;

        let body = json!({
            "error": "payment_required",
            "amount": state.config.pvp_price_str(),
            "currency": state.config.pvp_currency,
        });

        return Ok((
            StatusCode::PAYMENT_REQUIRED,
            [
                ("www-authenticate", www_auth),
                ("cache-control", "no-store".to_string()),
            ],
            Json(body),
        )
            .into_response());
    };

    let (user, receipt) = resolve_user_with_receipt(&state, auth).await?;

    let mut tx = state.db.begin().await?;

    // Check if already in queue
    if let Some(existing) = db::matchmaking::find_by_user(&mut *tx, user.id).await? {
        tx.commit().await?;
        return Ok(Json(PvpQueueResponse {
            game_id: existing.pvp_game_id,
            status: "queued".to_string(),
            matched: false,
        })
        .into_response());
    }

    // Try to find a match
    let match_found = db::matchmaking::find_match(
        &mut *tx,
        state.config.pvp_price,
        &state.config.pvp_currency,
        user.id,
    )
    .await?;

    if let Some(opponent_entry) = match_found {
        // Match found! Join the opponent's game as player2
        let game = db::pvp_games::lock_for_update(&mut *tx, opponent_entry.pvp_game_id)
            .await?
            .ok_or_else(|| AppError::Internal(anyhow::anyhow!("matched game not found")))?;

        db::pvp_games::set_player2(&mut *tx, game.id, user.id, PvpGameStatus::BothPaid)
            .await?;
        db::pvp_games::set_both_paid(&mut *tx, game.id).await?;

        // Record player2 payment
        db::pvp_payments::create(
            &mut *tx,
            db::pvp_payments::CreatePvpPaymentParams {
                pvp_game_id: game.id,
                player_id: user.id,
                amount: game.price,
                provider_payment_id: Some(receipt.reference.clone()),
                authorization_payload: None,
                receipt_payload: None,
            },
        )
        .await?;

        // Remove opponent from queue
        db::matchmaking::remove(&mut *tx, opponent_entry.user_id).await?;

        tx.commit().await?;

        tracing::info!(
            game_id = %game.id,
            player1 = ?game.player1_id,
            player2 = %user.wallet_address,
            "matchmaking matched"
        );

        let receipt_header =
            format_receipt(&receipt).unwrap_or_else(|_| receipt.reference.clone());

        return Ok((
            StatusCode::OK,
            [("payment-receipt", receipt_header)],
            Json(PvpQueueResponse {
                game_id: game.id,
                status: "both_paid".to_string(),
                matched: true,
            }),
        )
            .into_response());
    }

    // No match: create a new game and queue
    let game_id = Uuid::new_v4();
    let game = db::pvp_games::create(
        &mut *tx,
        db::pvp_games::CreatePvpGameParams {
            id: game_id,
            room_code: None, // matchmaking games don't have room codes
            player1_id: user.id,
            price: state.config.pvp_price,
            currency: state.config.pvp_currency.clone(),
            platform_fee_bps: state.config.pvp_platform_fee_bps,
        },
    )
    .await?;

    // Record payment
    db::pvp_payments::create(
        &mut *tx,
        db::pvp_payments::CreatePvpPaymentParams {
            pvp_game_id: game.id,
            player_id: user.id,
            amount: game.price,
            provider_payment_id: Some(receipt.reference.clone()),
            authorization_payload: None,
            receipt_payload: None,
        },
    )
    .await?;

    db::pvp_games::transition(&mut *tx, game.id, PvpGameStatus::Player1Paid).await?;

    db::matchmaking::enqueue(
        &mut *tx,
        user.id,
        game.id,
        state.config.pvp_price,
        &state.config.pvp_currency,
    )
    .await?;

    tx.commit().await?;

    tracing::info!(game_id = %game.id, "player queued for matchmaking");

    let receipt_header =
        format_receipt(&receipt).unwrap_or_else(|_| receipt.reference.clone());

    Ok((
        StatusCode::ACCEPTED,
        [("payment-receipt", receipt_header)],
        Json(PvpQueueResponse {
            game_id: game.id,
            status: "queued".to_string(),
            matched: false,
        }),
    )
        .into_response())
}

// ================================
// DELETE /api/pvp/queue
// ================================

async fn handle_dequeue(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> Result<Response, AppError> {
    let auth_header = headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .ok_or_else(|| AppError::PaymentInvalid("missing authorization".to_string()))?;

    let user = resolve_user(&state, auth_header).await?;

    let removed = db::matchmaking::remove(&state.db, user.id).await?;

    if removed == 0 {
        return Err(AppError::NotFound("not in queue".to_string()));
    }

    Ok(Json(json!({ "status": "removed" })).into_response())
}

// ================================
// POST /api/pvp/commit/{game_id}
// ================================

async fn handle_commit(
    State(state): State<Arc<AppState>>,
    Path(game_id): Path<Uuid>,
    headers: HeaderMap,
    Json(req): Json<PvpCommitRequest>,
) -> Result<Json<PvpCommitResponse>, AppError> {
    let auth_header = headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .ok_or_else(|| AppError::PaymentInvalid("missing authorization".to_string()))?;

    let user = resolve_user(&state, auth_header).await?;

    let mut tx = state.db.begin().await?;

    let game = db::pvp_games::lock_for_update(&mut *tx, game_id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("game {game_id} not found")))?;

    if !game.status.is_waiting_commit() {
        return Err(AppError::Conflict(format!(
            "game is not accepting commits (status: {:?})",
            game.status
        )));
    }

    let player_number = game
        .player_number(user.id)
        .ok_or_else(|| AppError::Validation("you are not a player in this game".to_string()))?;

    // Check if this player already committed
    let already_committed = match player_number {
        1 => game.player1_commit.is_some(),
        2 => game.player2_commit.is_some(),
        _ => false,
    };
    if already_committed {
        return Err(AppError::Conflict("already committed".to_string()));
    }

    let new_status = pvp_game::status_after_commit(&game, player_number)
        .ok_or_else(|| AppError::Conflict("invalid commit transition".to_string()))?;

    db::pvp_games::set_player_commit(&mut *tx, game.id, player_number, &req.commit, new_status)
        .await?;

    if new_status == PvpGameStatus::BothCommitted {
        db::pvp_games::set_both_committed(&mut *tx, game.id).await?;
    }

    tx.commit().await?;

    tracing::info!(
        game_id = %game_id,
        player = player_number,
        "player committed"
    );

    Ok(Json(PvpCommitResponse {
        game_id,
        status: format!("{new_status:?}").to_lowercase(),
        round: game.current_round,
    }))
}

// ================================
// POST /api/pvp/reveal/{game_id}
// ================================

async fn handle_reveal(
    State(state): State<Arc<AppState>>,
    Path(game_id): Path<Uuid>,
    headers: HeaderMap,
    Json(req): Json<PvpRevealRequest>,
) -> Result<Json<PvpRevealResponse>, AppError> {
    let auth_header = headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .ok_or_else(|| AppError::PaymentInvalid("missing authorization".to_string()))?;

    let user = resolve_user(&state, auth_header).await?;

    let mut tx = state.db.begin().await?;

    let game = db::pvp_games::lock_for_update(&mut *tx, game_id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("game {game_id} not found")))?;

    if !game.status.is_waiting_reveal() {
        return Err(AppError::Conflict(format!(
            "game is not accepting reveals (status: {:?})",
            game.status
        )));
    }

    let player_number = game
        .player_number(user.id)
        .ok_or_else(|| AppError::Validation("you are not a player in this game".to_string()))?;

    // Check if already revealed
    let already_revealed = match player_number {
        1 => game.player1_choice.is_some(),
        2 => game.player2_choice.is_some(),
        _ => false,
    };
    if already_revealed {
        return Err(AppError::Conflict("already revealed".to_string()));
    }

    // Verify commit
    let expected_commit = match player_number {
        1 => game
            .player1_commit
            .as_ref()
            .ok_or_else(|| AppError::Internal(anyhow::anyhow!("player1 commit missing")))?,
        2 => game
            .player2_commit
            .as_ref()
            .ok_or_else(|| AppError::Internal(anyhow::anyhow!("player2 commit missing")))?,
        _ => return Err(AppError::Internal(anyhow::anyhow!("invalid player number"))),
    };

    if !fairness::verify_commit(&game.id, &req.choice, &req.salt, expected_commit) {
        return Err(AppError::Validation(
            "reveal does not match commit".to_string(),
        ));
    }

    let is_both_revealed = pvp_game::both_revealed(&game, player_number);

    if !is_both_revealed {
        // Only this player revealed — waiting for opponent
        let new_status = pvp_game::status_after_reveal(&game, player_number)
            .unwrap_or(if player_number == 1 {
                PvpGameStatus::Player1Revealed
            } else {
                PvpGameStatus::Player2Revealed
            });

        db::pvp_games::set_player_reveal(
            &mut *tx,
            game.id,
            player_number,
            &req.choice,
            &req.salt,
            new_status,
        )
        .await?;

        tx.commit().await?;

        return Ok(Json(PvpRevealResponse {
            game_id,
            status: format!("{new_status:?}").to_lowercase(),
            result: None,
        }));
    }

    // Both revealed — resolve the game
    // First, store this player's reveal
    db::pvp_games::set_player_reveal(
        &mut *tx,
        game.id,
        player_number,
        &req.choice,
        &req.salt,
        game.status, // keep current status temporarily
    )
    .await?;

    // Get both choices
    let (p1_choice, p2_choice) = match player_number {
        1 => (
            req.choice,
            game.player2_choice
                .ok_or_else(|| AppError::Internal(anyhow::anyhow!("player2 choice missing")))?,
        ),
        2 => (
            game.player1_choice
                .ok_or_else(|| AppError::Internal(anyhow::anyhow!("player1 choice missing")))?,
            req.choice,
        ),
        _ => return Err(AppError::Internal(anyhow::anyhow!("invalid player number"))),
    };

    let outcome = pvp_game::resolve_pvp(&p1_choice, &p2_choice);

    // Build round result
    let round_result = pvp_game::PvpRoundResult {
        round: game.current_round,
        player1_choice: p1_choice,
        player1_commit: game.player1_commit.clone().unwrap_or_default(),
        player2_choice: p2_choice,
        player2_commit: game.player2_commit.clone().unwrap_or_default(),
        result: outcome,
    };

    let mut rounds: Vec<pvp_game::PvpRoundResult> =
        serde_json::from_value(game.rounds.clone()).unwrap_or_default();
    rounds.push(round_result);
    let rounds_json = serde_json::to_value(&rounds)
        .map_err(|e| AppError::Internal(anyhow::anyhow!("serialize rounds: {e}")))?;

    if outcome == PvpOutcome::Draw && game.current_round < pvp_game::MAX_REMATCH_ROUNDS {
        // Draw → reset for rematch
        let new_round = game.current_round + 1;

        // Save rounds first, then reset
        sqlx::query("UPDATE pvp_games SET rounds = $2 WHERE id = $1")
            .bind(game.id)
            .bind(&rounds_json)
            .execute(&mut *tx)
            .await?;

        db::pvp_games::reset_for_rematch(&mut *tx, game.id, new_round).await?;

        tx.commit().await?;

        tracing::info!(game_id = %game_id, round = new_round, "pvp draw, rematch");

        return Ok(Json(PvpRevealResponse {
            game_id,
            status: "both_paid".to_string(), // back to commit phase
            result: None,
        }));
    }

    // Final resolution
    let resolved_status = pvp_game::status_for_outcome(&outcome);
    db::pvp_games::resolve(&mut *tx, game.id, outcome, resolved_status, &rounds_json).await?;

    // Load the updated game for settlement
    let resolved_game = db::pvp_games::find_by_id(&mut *tx, game.id)
        .await?
        .ok_or_else(|| AppError::Internal(anyhow::anyhow!("game vanished after resolve")))?;

    // Settle inline
    let plan = pvp_settlement::PvpSettlementPlan::from_game(&resolved_game, &outcome);

    db::pvp_games::transition(&mut *tx, game.id, PvpGameStatus::Settling).await?;

    let settlement = db::pvp_settlements::create(
        &mut *tx,
        db::pvp_settlements::CreatePvpSettlementParams {
            pvp_game_id: game.id,
            result: plan.result,
            pot_amount: plan.pot_amount,
            platform_fee: plan.platform_fee,
            winner_payout: plan.winner_payout,
            loser_refund: plan.loser_refund,
            winner_id: plan.winner_id,
            reward_token: plan.reward_token,
            reward_amount: plan.reward_amount,
        },
    )
    .await?;

    if let (Some(token), Some(winner)) = (plan.reward_token, plan.winner_id) {
        if plan.reward_amount > 0 {
            db::inventories::credit(&mut *tx, winner, token, plan.reward_amount).await?;
        }
    }

    db::pvp_games::mark_settled(&mut *tx, game.id).await?;
    db::matchmaking::remove_by_game(&mut *tx, game.id).await?;

    tx.commit().await?;

    tracing::info!(
        game_id = %game_id,
        outcome = ?outcome,
        rounds = rounds.len(),
        "pvp game resolved and settled"
    );

    // Build response relative to the requesting player
    let (your_choice, opponent_choice) = match player_number {
        1 => (p1_choice, p2_choice),
        _ => (p2_choice, p1_choice),
    };

    Ok(Json(PvpRevealResponse {
        game_id,
        status: "settled".to_string(),
        result: Some(PvpResultDetail {
            outcome,
            your_choice,
            opponent_choice,
            settlement: PvpSettlementSummary {
                pot_amount: plan.pot_amount.to_string(),
                platform_fee: plan.platform_fee.to_string(),
                winner_payout: plan.winner_payout.to_string(),
                reward_token: plan.reward_token,
                reward_amount: plan.reward_amount,
            },
            receipt_id: settlement.id,
        }),
    }))
}

// ================================
// GET /api/pvp/game/{game_id}
// ================================

async fn handle_game_detail(
    State(state): State<Arc<AppState>>,
    Path(game_id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<PvpGameDetailResponse>, AppError> {
    let game = db::pvp_games::find_by_id(&state.db, game_id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("game {game_id} not found")))?;

    // Try to identify the requesting player
    let player_number = identify_player(&state, &headers, &game).await;

    let (your_choice, opponent_choice) = match player_number {
        Some(1) if game.status.is_terminal() || game.status.is_resolved() => {
            (game.player1_choice, game.player2_choice)
        }
        Some(2) if game.status.is_terminal() || game.status.is_resolved() => {
            (game.player2_choice, game.player1_choice)
        }
        Some(1) => (game.player1_choice, None),
        Some(2) => (game.player2_choice, None),
        _ => (None, None),
    };

    let settlement = if game.status == PvpGameStatus::Settled {
        db::pvp_settlements::find_by_game_id(&state.db, game.id)
            .await?
            .map(|s| PvpSettlementSummary {
                pot_amount: s.pot_amount.to_string(),
                platform_fee: s.platform_fee.to_string(),
                winner_payout: s.winner_payout.to_string(),
                reward_token: s.reward_token,
                reward_amount: s.reward_amount,
            })
    } else {
        None
    };

    Ok(Json(PvpGameDetailResponse {
        id: game.id,
        room_code: game.room_code,
        status: format!("{:?}", game.status).to_lowercase(),
        price: game.price.to_string(),
        currency: game.currency,
        current_round: game.current_round,
        your_player_number: player_number,
        your_choice,
        opponent_choice,
        result: game.result,
        settlement,
        created_at: game.created_at.to_rfc3339(),
        resolved_at: game.resolved_at.map(|t| t.to_rfc3339()),
        settled_at: game.settled_at.map(|t| t.to_rfc3339()),
    }))
}

/// Best-effort player identification from auth header.
async fn identify_player(
    state: &Arc<AppState>,
    headers: &HeaderMap,
    game: &PvpGameRow,
) -> Option<u8> {
    let auth = headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())?;

    let user = resolve_user(state, auth).await.ok()?;
    game.player_number(user.id)
}
