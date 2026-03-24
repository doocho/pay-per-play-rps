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
        WITH combined AS (
            -- PvE games
            SELECT
                g.user_id,
                CASE WHEN g.result = 'win' THEN 1 ELSE 0 END AS is_win,
                CASE WHEN g.result = 'draw' THEN 1 ELSE 0 END AS is_draw,
                CASE WHEN g.result = 'lose' THEN 1 ELSE 0 END AS is_loss
            FROM games g
            WHERE g.status = 'settled' AND g.user_id IS NOT NULL

            UNION ALL

            -- PvP games (player1 perspective)
            SELECT
                pg.player1_id AS user_id,
                CASE WHEN pg.result = 'player1_wins' THEN 1 ELSE 0 END AS is_win,
                CASE WHEN pg.result = 'draw' THEN 1 ELSE 0 END AS is_draw,
                CASE WHEN pg.result = 'player2_wins' THEN 1 ELSE 0 END AS is_loss
            FROM pvp_games pg
            WHERE pg.status = 'settled' AND pg.player1_id IS NOT NULL

            UNION ALL

            -- PvP games (player2 perspective)
            SELECT
                pg.player2_id AS user_id,
                CASE WHEN pg.result = 'player2_wins' THEN 1 ELSE 0 END AS is_win,
                CASE WHEN pg.result = 'draw' THEN 1 ELSE 0 END AS is_draw,
                CASE WHEN pg.result = 'player1_wins' THEN 1 ELSE 0 END AS is_loss
            FROM pvp_games pg
            WHERE pg.status = 'settled' AND pg.player2_id IS NOT NULL
        )
        SELECT
            u.wallet_address,
            COUNT(*) AS total_games,
            SUM(c.is_win)::bigint AS wins,
            SUM(c.is_draw)::bigint AS draws,
            SUM(c.is_loss)::bigint AS losses
        FROM combined c
        JOIN users u ON u.id = c.user_id
        GROUP BY u.wallet_address
        ORDER BY wins DESC, total_games DESC
        LIMIT 50
        "#,
    )
    .fetch_all(&state.db)
    .await?;

    Ok(Json(entries))
}
