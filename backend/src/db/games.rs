use chrono::Utc;
use rust_decimal::Decimal;
use sqlx::{PgExecutor, PgPool};
use uuid::Uuid;

use crate::types::domain::{Choice, GameRow, GameStatus, Outcome};

pub struct CreateGameParams {
    pub id: Option<Uuid>,
    pub price: Decimal,
    pub currency: String,
    pub user_choice: Choice,
    pub server_choice: Choice,
    pub server_salt: String,
    pub server_commit: String,
    pub rounds: serde_json::Value,
}

pub async fn create(pool: &PgPool, params: CreateGameParams) -> Result<GameRow, sqlx::Error> {
    create_with_tx(pool, params).await
}

pub async fn create_with_tx(
    executor: impl PgExecutor<'_>,
    params: CreateGameParams,
) -> Result<GameRow, sqlx::Error> {
    let id = params.id.unwrap_or_else(Uuid::new_v4);
    sqlx::query_as::<_, GameRow>(
        r#"
        INSERT INTO games (id, price, currency, user_choice, server_choice, server_salt, server_commit, rounds, status)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, 'payment_required')
        RETURNING *
        "#,
    )
    .bind(id)
    .bind(params.price)
    .bind(&params.currency)
    .bind(&params.user_choice)
    .bind(&params.server_choice)
    .bind(&params.server_salt)
    .bind(&params.server_commit)
    .bind(&params.rounds)
    .fetch_one(executor)
    .await
}

pub async fn find_by_id(pool: &PgPool, id: Uuid) -> Result<Option<GameRow>, sqlx::Error> {
    sqlx::query_as::<_, GameRow>("SELECT * FROM games WHERE id = $1")
        .bind(id)
        .fetch_optional(pool)
        .await
}

/// Acquires a row-level lock and returns the game only if it's in the expected status.
pub async fn lock_for_update(
    tx: impl PgExecutor<'_>,
    game_id: Uuid,
    expected_status: GameStatus,
) -> Result<Option<GameRow>, sqlx::Error> {
    sqlx::query_as::<_, GameRow>(
        "SELECT * FROM games WHERE id = $1 AND status = $2 FOR UPDATE",
    )
    .bind(game_id)
    .bind(&expected_status)
    .fetch_optional(tx)
    .await
}

pub async fn transition(
    tx: impl PgExecutor<'_>,
    game_id: Uuid,
    new_status: GameStatus,
) -> Result<GameRow, sqlx::Error> {
    sqlx::query_as::<_, GameRow>(
        "UPDATE games SET status = $2 WHERE id = $1 RETURNING *",
    )
    .bind(game_id)
    .bind(&new_status)
    .fetch_one(tx)
    .await
}

pub async fn set_user_id(
    tx: impl PgExecutor<'_>,
    game_id: Uuid,
    user_id: Uuid,
) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE games SET user_id = $2 WHERE id = $1")
        .bind(game_id)
        .bind(user_id)
        .execute(tx)
        .await?;
    Ok(())
}

pub async fn resolve(
    tx: impl PgExecutor<'_>,
    game_id: Uuid,
    result: Outcome,
    status: GameStatus,
) -> Result<GameRow, sqlx::Error> {
    sqlx::query_as::<_, GameRow>(
        r#"
        UPDATE games SET status = $2, result = $3, resolved_at = $4
        WHERE id = $1 RETURNING *
        "#,
    )
    .bind(game_id)
    .bind(&status)
    .bind(&result)
    .bind(Utc::now())
    .fetch_one(tx)
    .await
}

/// Resolve game with auto-rematch round data, updating the final server choice/salt/commit.
pub async fn resolve_with_rounds(
    tx: impl PgExecutor<'_>,
    game_id: Uuid,
    result: Outcome,
    status: GameStatus,
    server_choice: &Choice,
    server_salt: &str,
    server_commit: &str,
    rounds: &serde_json::Value,
) -> Result<GameRow, sqlx::Error> {
    sqlx::query_as::<_, GameRow>(
        r#"
        UPDATE games SET status = $2, result = $3, resolved_at = $4,
            server_choice = $5, server_salt = $6, server_commit = $7, rounds = $8
        WHERE id = $1 RETURNING *
        "#,
    )
    .bind(game_id)
    .bind(&status)
    .bind(&result)
    .bind(Utc::now())
    .bind(server_choice)
    .bind(server_salt)
    .bind(server_commit)
    .bind(rounds)
    .fetch_one(tx)
    .await
}

pub async fn mark_settled(
    tx: impl PgExecutor<'_>,
    game_id: Uuid,
) -> Result<GameRow, sqlx::Error> {
    sqlx::query_as::<_, GameRow>(
        r#"
        UPDATE games SET status = 'settled', settled_at = $2
        WHERE id = $1 RETURNING *
        "#,
    )
    .bind(game_id)
    .bind(Utc::now())
    .fetch_one(tx)
    .await
}

pub async fn expire_stale(pool: &PgPool, ttl_seconds: u64) -> Result<u64, sqlx::Error> {
    let result = sqlx::query(
        r#"
        UPDATE games SET status = 'expired'
        WHERE status = 'payment_required'
          AND created_at < now() - make_interval(secs => $1::float8)
        "#,
    )
    .bind(ttl_seconds as f64)
    .execute(pool)
    .await?;
    Ok(result.rows_affected())
}

pub async fn find_stuck_resolved(pool: &PgPool) -> Result<Vec<GameRow>, sqlx::Error> {
    sqlx::query_as::<_, GameRow>(
        r#"
        SELECT * FROM games
        WHERE status IN ('resolved_win', 'resolved_draw', 'resolved_lose')
          AND resolved_at < now() - interval '30 seconds'
        "#,
    )
    .fetch_all(pool)
    .await
}

pub async fn find_by_user(pool: &PgPool, user_id: Uuid) -> Result<Vec<GameRow>, sqlx::Error> {
    sqlx::query_as::<_, GameRow>(
        "SELECT * FROM games WHERE user_id = $1 ORDER BY created_at DESC LIMIT 50",
    )
    .bind(user_id)
    .fetch_all(pool)
    .await
}
