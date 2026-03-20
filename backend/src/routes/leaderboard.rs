use std::sync::Arc;

use axum::extract::State;
use axum::routing::get;
use axum::{Json, Router};

use crate::app::AppState;
use crate::error::AppError;
use crate::types::api::LeaderboardEntry;

pub fn router() -> Router<Arc<AppState>> {
    Router::new().route("/leaderboard", get(get_leaderboard))
}

async fn get_leaderboard(
    State(state): State<Arc<AppState>>,
) -> Result<Json<Vec<LeaderboardEntry>>, AppError> {
    let entries = sqlx::query_as::<_, LeaderboardEntry>(
        r#"
        SELECT
            u.wallet_address,
            COUNT(*) AS total_games,
            COUNT(*) FILTER (WHERE g.result = 'win') AS wins,
            COUNT(*) FILTER (WHERE g.result = 'draw') AS draws,
            COUNT(*) FILTER (WHERE g.result = 'lose') AS losses
        FROM games g
        JOIN users u ON u.id = g.user_id
        WHERE g.status = 'settled'
        GROUP BY u.wallet_address
        ORDER BY wins DESC, total_games DESC
        LIMIT 50
        "#,
    )
    .fetch_all(&state.db)
    .await?;

    Ok(Json(entries))
}
