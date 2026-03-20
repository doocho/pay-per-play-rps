use std::sync::Arc;

use axum::extract::{Path, State};
use axum::routing::get;
use axum::{Json, Router};
use uuid::Uuid;

use crate::app::AppState;
use crate::db;
use crate::domain::fairness as fairness_domain;
use crate::domain::game::RoundResult;
use crate::error::AppError;
use crate::types::api::{FairnessResponse, RoundFairnessResult};
use crate::types::domain::GameStatus;

pub fn router() -> Router<Arc<AppState>> {
    Router::new().route("/fairness/{game_id}", get(verify_fairness))
}

async fn verify_fairness(
    State(state): State<Arc<AppState>>,
    Path(game_id): Path<Uuid>,
) -> Result<Json<FairnessResponse>, AppError> {
    let game = db::games::find_by_id(&state.db, game_id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("game {game_id} not found")))?;

    let is_revealed = matches!(
        game.status,
        GameStatus::ResolvedWin
            | GameStatus::ResolvedDraw
            | GameStatus::ResolvedLose
            | GameStatus::Settling
            | GameStatus::Settled
    );

    if !is_revealed {
        return Err(AppError::Validation(
            "fairness data is only available after game resolution".to_string(),
        ));
    }

    let stored_rounds: Vec<RoundResult> =
        serde_json::from_value(game.rounds.clone()).unwrap_or_default();

    if stored_rounds.is_empty() {
        // Legacy game with no rounds data — verify top-level fields only
        let recomputed =
            fairness_domain::compute_commit(&game.id, &game.server_choice, &game.server_salt);
        let verified = recomputed == game.server_commit;

        return Ok(Json(FairnessResponse {
            game_id: game.id,
            total_rounds: 1,
            all_verified: verified,
            rounds: vec![RoundFairnessResult {
                round: 1,
                server_choice: game.server_choice,
                server_salt: game.server_salt,
                original_commit: game.server_commit,
                recomputed_commit: recomputed,
                verified,
            }],
        }));
    }

    let mut round_results = Vec::with_capacity(stored_rounds.len());
    let mut all_verified = true;

    for r in &stored_rounds {
        let recomputed =
            fairness_domain::compute_commit(&game.id, &r.server_choice, &r.server_salt);
        let verified = recomputed == r.server_commit;
        if !verified {
            all_verified = false;
        }
        round_results.push(RoundFairnessResult {
            round: r.round,
            server_choice: r.server_choice,
            server_salt: r.server_salt.clone(),
            original_commit: r.server_commit.clone(),
            recomputed_commit: recomputed,
            verified,
        });
    }

    Ok(Json(FairnessResponse {
        game_id: game.id,
        total_rounds: round_results.len(),
        all_verified,
        rounds: round_results,
    }))
}
