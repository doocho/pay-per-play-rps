use sqlx::PgPool;
use std::time::Duration;

use crate::config::AppConfig;

pub fn spawn_all(db: PgPool, config: &AppConfig) {
    spawn_game_expiration(db.clone(), config.game_ttl_seconds);
    spawn_idempotency_cleanup(db.clone());
    spawn_settlement_retry(db.clone());
    spawn_pvp_timeout(
        db.clone(),
        config.pvp_payment_timeout_seconds,
        config.pvp_commit_timeout_seconds,
        config.pvp_reveal_timeout_seconds,
    );
    spawn_pvp_settlement_retry(db);
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

fn spawn_pvp_timeout(
    db: PgPool,
    payment_timeout: u64,
    commit_timeout: u64,
    reveal_timeout: u64,
) {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(15));
        loop {
            interval.tick().await;
            match crate::db::pvp_games::find_timed_out(
                &db,
                payment_timeout as f64,
                commit_timeout as f64,
                reveal_timeout as f64,
            )
            .await
            {
                Ok(games) => {
                    for game in games {
                        tracing::info!(game_id = %game.id, status = ?game.status, "expiring timed-out pvp game");
                        if let Err(e) = crate::db::pvp_games::transition(
                            &db,
                            game.id,
                            crate::types::pvp::PvpGameStatus::Expired,
                        )
                        .await
                        {
                            tracing::error!(game_id = %game.id, error = %e, "pvp expiration failed");
                        }
                        // Clean up matchmaking queue
                        let _ = crate::db::matchmaking::remove_by_game(&db, game.id).await;
                    }
                }
                Err(e) => {
                    tracing::error!(error = %e, "pvp timeout sweep failed");
                }
            }
        }
    });
}

fn spawn_pvp_settlement_retry(db: PgPool) {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(30));
        loop {
            interval.tick().await;
            match crate::db::pvp_games::find_stuck_resolved(&db).await {
                Ok(games) => {
                    for game in games {
                        tracing::info!(game_id = %game.id, "retrying stuck pvp settlement");
                        if let Err(e) =
                            crate::domain::pvp_settlement::execute(&db, &game).await
                        {
                            tracing::error!(
                                game_id = %game.id,
                                error = %e,
                                "pvp settlement retry failed"
                            );
                        }
                    }
                }
                Err(e) => {
                    tracing::error!(error = %e, "pvp settlement retry query failed");
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
