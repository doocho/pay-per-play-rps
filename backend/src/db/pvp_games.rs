use chrono::Utc;
use rust_decimal::Decimal;
use sqlx::PgExecutor;
use uuid::Uuid;

use crate::types::domain::Choice;
use crate::types::pvp::{PvpGameRow, PvpGameStatus, PvpOutcome};

pub struct CreatePvpGameParams {
    pub id: Uuid,
    pub room_code: Option<String>,
    pub player1_id: Uuid,
    pub price: Decimal,
    pub currency: String,
    pub platform_fee_bps: i32,
}

pub async fn create(
    executor: impl PgExecutor<'_>,
    params: CreatePvpGameParams,
) -> Result<PvpGameRow, sqlx::Error> {
    sqlx::query_as::<_, PvpGameRow>(
        r#"
        INSERT INTO pvp_games (id, room_code, player1_id, price, currency, platform_fee_bps, status)
        VALUES ($1, $2, $3, $4, $5, $6, 'waiting_for_opponent')
        RETURNING *
        "#,
    )
    .bind(params.id)
    .bind(&params.room_code)
    .bind(params.player1_id)
    .bind(params.price)
    .bind(&params.currency)
    .bind(params.platform_fee_bps)
    .fetch_one(executor)
    .await
}

pub async fn find_by_id(
    executor: impl PgExecutor<'_>,
    id: Uuid,
) -> Result<Option<PvpGameRow>, sqlx::Error> {
    sqlx::query_as::<_, PvpGameRow>("SELECT * FROM pvp_games WHERE id = $1")
        .bind(id)
        .fetch_optional(executor)
        .await
}

pub async fn find_by_room_code(
    executor: impl PgExecutor<'_>,
    room_code: &str,
) -> Result<Option<PvpGameRow>, sqlx::Error> {
    sqlx::query_as::<_, PvpGameRow>("SELECT * FROM pvp_games WHERE room_code = $1")
        .bind(room_code)
        .fetch_optional(executor)
        .await
}

pub async fn lock_for_update(
    executor: impl PgExecutor<'_>,
    game_id: Uuid,
) -> Result<Option<PvpGameRow>, sqlx::Error> {
    sqlx::query_as::<_, PvpGameRow>("SELECT * FROM pvp_games WHERE id = $1 FOR UPDATE")
        .bind(game_id)
        .fetch_optional(executor)
        .await
}

pub async fn set_player2(
    executor: impl PgExecutor<'_>,
    game_id: Uuid,
    player2_id: Uuid,
    new_status: PvpGameStatus,
) -> Result<PvpGameRow, sqlx::Error> {
    sqlx::query_as::<_, PvpGameRow>(
        r#"
        UPDATE pvp_games
        SET player2_id = $2, player2_joined_at = $3, status = $4
        WHERE id = $1
        RETURNING *
        "#,
    )
    .bind(game_id)
    .bind(player2_id)
    .bind(Utc::now())
    .bind(&new_status)
    .fetch_one(executor)
    .await
}

pub async fn transition(
    executor: impl PgExecutor<'_>,
    game_id: Uuid,
    new_status: PvpGameStatus,
) -> Result<PvpGameRow, sqlx::Error> {
    sqlx::query_as::<_, PvpGameRow>(
        "UPDATE pvp_games SET status = $2 WHERE id = $1 RETURNING *",
    )
    .bind(game_id)
    .bind(&new_status)
    .fetch_one(executor)
    .await
}

pub async fn set_both_paid(
    executor: impl PgExecutor<'_>,
    game_id: Uuid,
) -> Result<PvpGameRow, sqlx::Error> {
    sqlx::query_as::<_, PvpGameRow>(
        r#"
        UPDATE pvp_games
        SET status = 'both_paid', both_paid_at = $2
        WHERE id = $1
        RETURNING *
        "#,
    )
    .bind(game_id)
    .bind(Utc::now())
    .fetch_one(executor)
    .await
}

pub async fn set_player_commit(
    executor: impl PgExecutor<'_>,
    game_id: Uuid,
    player_number: u8,
    commit: &str,
    new_status: PvpGameStatus,
) -> Result<PvpGameRow, sqlx::Error> {
    let sql = if player_number == 1 {
        r#"
        UPDATE pvp_games
        SET player1_commit = $2, status = $3
        WHERE id = $1
        RETURNING *
        "#
    } else {
        r#"
        UPDATE pvp_games
        SET player2_commit = $2, status = $3
        WHERE id = $1
        RETURNING *
        "#
    };
    sqlx::query_as::<_, PvpGameRow>(sql)
        .bind(game_id)
        .bind(commit)
        .bind(&new_status)
        .fetch_one(executor)
        .await
}

pub async fn set_both_committed(
    executor: impl PgExecutor<'_>,
    game_id: Uuid,
) -> Result<PvpGameRow, sqlx::Error> {
    sqlx::query_as::<_, PvpGameRow>(
        r#"
        UPDATE pvp_games
        SET status = 'both_committed', both_committed_at = $2
        WHERE id = $1
        RETURNING *
        "#,
    )
    .bind(game_id)
    .bind(Utc::now())
    .fetch_one(executor)
    .await
}

pub async fn set_player_reveal(
    executor: impl PgExecutor<'_>,
    game_id: Uuid,
    player_number: u8,
    choice: &Choice,
    salt: &str,
    new_status: PvpGameStatus,
) -> Result<PvpGameRow, sqlx::Error> {
    let sql = if player_number == 1 {
        r#"
        UPDATE pvp_games
        SET player1_choice = $2, player1_salt = $3, status = $4
        WHERE id = $1
        RETURNING *
        "#
    } else {
        r#"
        UPDATE pvp_games
        SET player2_choice = $2, player2_salt = $3, status = $4
        WHERE id = $1
        RETURNING *
        "#
    };
    sqlx::query_as::<_, PvpGameRow>(sql)
        .bind(game_id)
        .bind(choice)
        .bind(salt)
        .bind(&new_status)
        .fetch_one(executor)
        .await
}

pub async fn resolve(
    executor: impl PgExecutor<'_>,
    game_id: Uuid,
    result: PvpOutcome,
    status: PvpGameStatus,
    rounds: &serde_json::Value,
) -> Result<PvpGameRow, sqlx::Error> {
    sqlx::query_as::<_, PvpGameRow>(
        r#"
        UPDATE pvp_games
        SET status = $2, result = $3, rounds = $4, resolved_at = $5
        WHERE id = $1
        RETURNING *
        "#,
    )
    .bind(game_id)
    .bind(&status)
    .bind(&result)
    .bind(rounds)
    .bind(Utc::now())
    .fetch_one(executor)
    .await
}

pub async fn mark_settled(
    executor: impl PgExecutor<'_>,
    game_id: Uuid,
) -> Result<PvpGameRow, sqlx::Error> {
    sqlx::query_as::<_, PvpGameRow>(
        r#"
        UPDATE pvp_games SET status = 'settled', settled_at = $2
        WHERE id = $1 RETURNING *
        "#,
    )
    .bind(game_id)
    .bind(Utc::now())
    .fetch_one(executor)
    .await
}

/// Clear commits and choices for a new rematch round.
pub async fn reset_for_rematch(
    executor: impl PgExecutor<'_>,
    game_id: Uuid,
    new_round: i32,
) -> Result<PvpGameRow, sqlx::Error> {
    sqlx::query_as::<_, PvpGameRow>(
        r#"
        UPDATE pvp_games
        SET player1_choice = NULL, player1_salt = NULL, player1_commit = NULL,
            player2_choice = NULL, player2_salt = NULL, player2_commit = NULL,
            both_committed_at = NULL, status = 'both_paid', current_round = $2
        WHERE id = $1
        RETURNING *
        "#,
    )
    .bind(game_id)
    .bind(new_round)
    .fetch_one(executor)
    .await
}

/// Find games that are stuck waiting and have timed out.
pub async fn find_timed_out(
    executor: impl PgExecutor<'_>,
    payment_timeout_secs: f64,
    commit_timeout_secs: f64,
    reveal_timeout_secs: f64,
) -> Result<Vec<PvpGameRow>, sqlx::Error> {
    sqlx::query_as::<_, PvpGameRow>(
        r#"
        SELECT * FROM pvp_games
        WHERE (
            -- Waiting for opponent or partial payment
            status IN ('waiting_for_opponent', 'player1_paid', 'player2_paid')
            AND created_at < now() - make_interval(secs => $1::float8)
        ) OR (
            -- Waiting for commits
            status IN ('both_paid', 'player1_committed', 'player2_committed')
            AND both_paid_at < now() - make_interval(secs => $2::float8)
        ) OR (
            -- Waiting for reveals
            status IN ('both_committed', 'player1_revealed', 'player2_revealed')
            AND both_committed_at < now() - make_interval(secs => $3::float8)
        )
        "#,
    )
    .bind(payment_timeout_secs)
    .bind(commit_timeout_secs)
    .bind(reveal_timeout_secs)
    .fetch_all(executor)
    .await
}

pub async fn find_stuck_resolved(
    executor: impl PgExecutor<'_>,
) -> Result<Vec<PvpGameRow>, sqlx::Error> {
    sqlx::query_as::<_, PvpGameRow>(
        r#"
        SELECT * FROM pvp_games
        WHERE status IN ('resolved_player1_wins', 'resolved_player2_wins', 'resolved_draw')
          AND resolved_at < now() - interval '30 seconds'
        "#,
    )
    .fetch_all(executor)
    .await
}
