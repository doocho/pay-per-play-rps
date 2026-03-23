use alloy::consensus::transaction::SignerRecoverable;
use alloy::network::TransactionResponse;
use alloy::primitives::B256;
use alloy::providers::Provider;
use mpp::parse_authorization;
use mpp::protocol::methods::tempo::{
    FeePayerEnvelope78, TEMPO_FEE_PAYER_ENVELOPE_TYPE_ID, TEMPO_TX_TYPE_ID,
};
use mpp::server::TempoProvider;
use tempo_primitives::AASigned;

use crate::error::AppError;

/// Resolve payer wallet: prefer **local** recovery from the signed tx in the credential
/// (`transaction` payload); if the client only sent a **hash** payload, fall back to RPC
/// (`eth_getTransactionByHash` using MPP [`mpp::Receipt::reference`]).
pub async fn resolve_payer_wallet(
    auth_header: &str,
    receipt_tx_hash: &str,
    provider: &TempoProvider,
) -> Result<String, AppError> {
    let credential = parse_authorization(auth_header).map_err(|e| {
        AppError::PaymentInvalid(format!("invalid Authorization header (parse): {e}"))
    })?;

    let payload = credential.charge_payload().map_err(|e| {
        AppError::PaymentInvalid(format!("charge credential payload: {e}"))
    })?;

    if payload.is_transaction() {
        let hex = payload.signed_tx().expect("is_transaction implies Some");
        return payer_from_signed_tx_hex(hex);
    }

    if payload.is_hash() {
        return payer_address_from_receipt_tx_hash(provider, receipt_tx_hash).await;
    }

    Err(AppError::Internal(anyhow::anyhow!(
        "unexpected payment payload type"
    )))
}

fn payer_from_signed_tx_hex(signed_tx_hex: &str) -> Result<String, AppError> {
    let s = signed_tx_hex.trim().strip_prefix("0x").unwrap_or(signed_tx_hex);
    let bytes = hex::decode(s).map_err(|e| {
        AppError::PaymentInvalid(format!("signed transaction is not valid hex: {e}"))
    })?;
    payer_from_signed_tx_bytes(&bytes)
}

fn payer_from_signed_tx_bytes(tx_bytes: &[u8]) -> Result<String, AppError> {
    if tx_bytes.is_empty() {
        return Err(AppError::PaymentInvalid(
            "signed transaction payload is empty".to_string(),
        ));
    }

    let addr = if tx_bytes[0] == TEMPO_FEE_PAYER_ENVELOPE_TYPE_ID {
        let env = FeePayerEnvelope78::decode_envelope(tx_bytes).map_err(|e| {
            AppError::PaymentInvalid(format!("fee payer envelope decode: {e}"))
        })?;
        env.to_recoverable_signed()
            .recover_signer()
            .map_err(|e| AppError::PaymentInvalid(format!("recover signer (0x78): {e}")))?
    } else {
        let tx_data = if tx_bytes[0] == TEMPO_TX_TYPE_ID {
            &tx_bytes[1..]
        } else {
            tx_bytes
        };
        let signed = AASigned::rlp_decode(&mut &tx_data[..]).map_err(|e| {
            AppError::PaymentInvalid(format!("decode Tempo signed tx: {e}"))
        })?;
        signed
            .recover_signer()
            .map_err(|e| AppError::PaymentInvalid(format!("recover signer (0x76): {e}")))?
    };

    Ok(format!("{addr:#x}"))
}

async fn payer_address_from_receipt_tx_hash(
    provider: &TempoProvider,
    tx_hash_hex: &str,
) -> Result<String, AppError> {
    let hash = tx_hash_hex
        .trim()
        .parse::<B256>()
        .map_err(|e| {
            AppError::Internal(anyhow::anyhow!(
                "invalid tx hash in payment receipt reference: {e}"
            ))
        })?;

    let tx = provider
        .get_transaction_by_hash(hash)
        .await
        .map_err(|e| {
            AppError::Internal(anyhow::anyhow!(
                "rpc eth_getTransactionByHash failed: {e}"
            ))
        })?
        .ok_or_else(|| {
            AppError::Internal(anyhow::anyhow!(
                "transaction {tx_hash_hex} not found (receipt reference)"
            ))
        })?;

    let from = tx.from();

    Ok(format!("{from:#x}"))
}
