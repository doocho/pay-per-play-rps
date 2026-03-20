use std::sync::Arc;

use axum::extract::{Path, State};
use axum::routing::get;
use axum::{Json, Router};
use uuid::Uuid;

use crate::app::AppState;
use crate::db;
use crate::error::AppError;
use crate::types::api::GameDetailResponse;
use crate::types::domain::GameStatus;

pub fn router() -> Router<Arc<AppState>> {
    Router::new().route("/games/{game_id}", get(get_game))
}

async fn get_game(
    State(state): State<Arc<AppState>>,
    Path(game_id): Path<Uuid>,
) -> Result<Json<GameDetailResponse>, AppError> {
    let game = db::games::find_by_id(&state.db, game_id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("game {game_id} not found")))?;

    let is_revealed = matches!(
        game.status,
        GameStatus::PlayLocked
            | GameStatus::ResolvedWin
            | GameStatus::ResolvedDraw
            | GameStatus::ResolvedLose
            | GameStatus::Settling
            | GameStatus::Settled
    );

    Ok(Json(GameDetailResponse {
        id: game.id,
        status: format!("{:?}", game.status).to_lowercase(),
        user_choice: game.user_choice,
        result: game.result,
        price: game.price.to_string(),
        currency: game.currency,
        server_choice: if is_revealed {
            Some(game.server_choice)
        } else {
            None
        },
        server_salt: if is_revealed {
            Some(game.server_salt)
        } else {
            None
        },
        server_commit: game.server_commit,
        created_at: game.created_at.to_rfc3339(),
        resolved_at: game.resolved_at.map(|t| t.to_rfc3339()),
        settled_at: game.settled_at.map(|t| t.to_rfc3339()),
    }))
}
