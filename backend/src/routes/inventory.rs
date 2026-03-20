use std::sync::Arc;

use axum::extract::{Path, State};
use axum::routing::get;
use axum::{Json, Router};

use crate::app::AppState;
use crate::error::AppError;
use crate::types::api::{InventoryResponse, TokenBalance};

pub fn router() -> Router<Arc<AppState>> {
    Router::new().route("/inventory/{wallet_address}", get(get_inventory))
}

async fn get_inventory(
    State(state): State<Arc<AppState>>,
    Path(wallet_address): Path<String>,
) -> Result<Json<InventoryResponse>, AppError> {
    let rows =
        crate::domain::inventory::get_balances_for_wallet(&state.db, &wallet_address).await?;

    let tokens = rows
        .into_iter()
        .map(|r| TokenBalance {
            token_type: r.token_type,
            balance: r.balance,
        })
        .collect();

    Ok(Json(InventoryResponse {
        wallet_address,
        tokens,
    }))
}
