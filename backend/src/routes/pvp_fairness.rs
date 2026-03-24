use std::sync::Arc;

use axum::extract::{Path, State};
use axum::routing::get;
use axum::{Json, Router};
use uuid::Uuid;

use crate::app::AppState;
use crate::db;
use crate::domain::fairness;
use crate::error::AppError;
use crate::types::pvp_api::PvpFairnessResponse;

pub fn router() -> Router<Arc<AppState>> {
    Router::new().route("/pvp/fairness/{game_id}", get(handle_pvp_fairness))
}

async fn handle_pvp_fairness(
    State(state): State<Arc<AppState>>,
    Path(game_id): Path<Uuid>,
) -> Result<Json<PvpFairnessResponse>, AppError> {
    let game = db::pvp_games::find_by_id(&state.db, game_id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("game {game_id} not found")))?;

    let p1_verified = match (&game.player1_choice, &game.player1_salt, &game.player1_commit) {
        (Some(choice), Some(salt), Some(commit)) => {
            Some(fairness::verify_commit(&game.id, choice, salt, commit))
        }
        _ => None,
    };

    let p2_verified = match (&game.player2_choice, &game.player2_salt, &game.player2_commit) {
        (Some(choice), Some(salt), Some(commit)) => {
            Some(fairness::verify_commit(&game.id, choice, salt, commit))
        }
        _ => None,
    };

    Ok(Json(PvpFairnessResponse {
        game_id,
        player1_commit: game.player1_commit,
        player1_choice: game.player1_choice,
        player1_salt: game.player1_salt,
        player1_verified: p1_verified,
        player2_commit: game.player2_commit,
        player2_choice: game.player2_choice,
        player2_salt: game.player2_salt,
        player2_verified: p2_verified,
    }))
}
