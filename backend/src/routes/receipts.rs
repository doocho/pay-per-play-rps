use std::sync::Arc;

use axum::extract::{Path, State};
use axum::routing::get;
use axum::{Json, Router};
use uuid::Uuid;

use crate::app::AppState;
use crate::db;
use crate::error::AppError;
use crate::types::api::ReceiptResponse;

pub fn router() -> Router<Arc<AppState>> {
    Router::new().route("/receipts/{receipt_id}", get(get_receipt))
}

async fn get_receipt(
    State(state): State<Arc<AppState>>,
    Path(receipt_id): Path<Uuid>,
) -> Result<Json<ReceiptResponse>, AppError> {
    let settlement = sqlx::query_as::<_, crate::types::domain::SettlementRow>(
        "SELECT * FROM settlements WHERE id = $1",
    )
    .bind(receipt_id)
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| AppError::NotFound(format!("receipt {receipt_id} not found")))?;

    let game = db::games::find_by_id(&state.db, settlement.game_id)
        .await?
        .ok_or_else(|| AppError::Internal(anyhow::anyhow!("game not found for settlement")))?;

    let payment = db::payments::find_by_game_id(&state.db, game.id).await?;

    Ok(Json(ReceiptResponse {
        receipt_id: settlement.id,
        game_id: game.id,
        outcome: settlement.outcome,
        payment_amount: payment.map(|p| p.amount.to_string()).unwrap_or_default(),
        refund_amount: settlement.refund_amount.to_string(),
        captured_amount: settlement.captured_amount.to_string(),
        reward_token: settlement.reward_token,
        reward_amount: settlement.reward_amount,
        settled_at: game.settled_at.map(|t| t.to_rfc3339()),
    }))
}
