use rust_decimal::Decimal;
use sqlx::PgExecutor;
use uuid::Uuid;

use crate::types::pvp::MatchmakingQueueRow;

pub async fn enqueue(
    executor: impl PgExecutor<'_>,
    user_id: Uuid,
    pvp_game_id: Uuid,
    price: Decimal,
    currency: &str,
) -> Result<MatchmakingQueueRow, sqlx::Error> {
    sqlx::query_as::<_, MatchmakingQueueRow>(
        r#"
        INSERT INTO matchmaking_queue (user_id, pvp_game_id, price, currency)
        VALUES ($1, $2, $3, $4)
        RETURNING *
        "#,
    )
    .bind(user_id)
    .bind(pvp_game_id)
    .bind(price)
    .bind(currency)
    .fetch_one(executor)
    .await
}

/// Find the oldest queued player with matching price/currency, excluding the given user.
pub async fn find_match(
    executor: impl PgExecutor<'_>,
    price: Decimal,
    currency: &str,
    exclude_user_id: Uuid,
) -> Result<Option<MatchmakingQueueRow>, sqlx::Error> {
    sqlx::query_as::<_, MatchmakingQueueRow>(
        r#"
        SELECT * FROM matchmaking_queue
        WHERE price = $1 AND currency = $2 AND user_id != $3
        ORDER BY enqueued_at ASC
        LIMIT 1
        FOR UPDATE SKIP LOCKED
        "#,
    )
    .bind(price)
    .bind(currency)
    .bind(exclude_user_id)
    .fetch_optional(executor)
    .await
}

pub async fn remove(
    executor: impl PgExecutor<'_>,
    user_id: Uuid,
) -> Result<u64, sqlx::Error> {
    let result = sqlx::query("DELETE FROM matchmaking_queue WHERE user_id = $1")
        .bind(user_id)
        .execute(executor)
        .await?;
    Ok(result.rows_affected())
}

pub async fn find_by_user(
    executor: impl PgExecutor<'_>,
    user_id: Uuid,
) -> Result<Option<MatchmakingQueueRow>, sqlx::Error> {
    sqlx::query_as::<_, MatchmakingQueueRow>(
        "SELECT * FROM matchmaking_queue WHERE user_id = $1",
    )
    .bind(user_id)
    .fetch_optional(executor)
    .await
}

/// Clean up stale queue entries (e.g., for expired games).
pub async fn remove_by_game(
    executor: impl PgExecutor<'_>,
    pvp_game_id: Uuid,
) -> Result<u64, sqlx::Error> {
    let result = sqlx::query("DELETE FROM matchmaking_queue WHERE pvp_game_id = $1")
        .bind(pvp_game_id)
        .execute(executor)
        .await?;
    Ok(result.rows_affected())
}
