use sqlx::PgPool;
use std::time::Duration;

use crate::config::AppConfig;

pub fn spawn_all(db: PgPool, config: &AppConfig) {
    spawn_game_expiration(db.clone(), config.game_ttl_seconds);
    spawn_idempotency_cleanup(db.clone());
    spawn_settlement_retry(db);
}

fn spawn_game_expiration(db: PgPool, ttl_seconds: u64) {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(60));
        loop {
            interval.tick().await;
            match crate::db::games::expire_stale(&db, ttl_seconds).await {
                Ok(count) => {
                    if count > 0 {
                        tracing::info!(expired = count, "expired stale games");
                    }
                }
                Err(e) => {
                    tracing::error!(error = %e, "game expiration sweep failed");
                }
            }
        }
    });
}

fn spawn_idempotency_cleanup(db: PgPool) {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(300));
        loop {
            interval.tick().await;
            match sqlx::query("DELETE FROM idempotency WHERE expires_at < now()")
                .execute(&db)
                .await
            {
                Ok(result) => {
                    let count = result.rows_affected();
                    if count > 0 {
                        tracing::info!(deleted = count, "cleaned up expired idempotency keys");
                    }
                }
                Err(e) => {
                    tracing::error!(error = %e, "idempotency cleanup failed");
                }
            }
        }
    });
}

fn spawn_settlement_retry(db: PgPool) {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(30));
        loop {
            interval.tick().await;
            match crate::db::games::find_stuck_resolved(&db).await {
                Ok(games) => {
                    for game in games {
                        tracing::info!(game_id = %game.id, "retrying stuck settlement");
                        if let Err(e) =
                            crate::domain::settlement::execute(&db, &game).await
                        {
                            tracing::error!(
                                game_id = %game.id,
                                error = %e,
                                "settlement retry failed"
                            );
                        }
                    }
                }
                Err(e) => {
                    tracing::error!(error = %e, "settlement retry query failed");
                }
            }
        }
    });
}
