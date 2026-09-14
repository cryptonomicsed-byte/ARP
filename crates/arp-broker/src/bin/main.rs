//! arp-broker — Action Receipt Protocol v1 HTTP server.
//!
//! Config (env):
//!   ARP_PORT      — listen port (default 7795)
//!   VANTAGE_URL   — optional: forward receipts to Vantage on submit

use std::sync::Arc;

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use serde::Deserialize;
use serde_json::json;

use arp_broker::ReceiptStore;
use arp_types::ActionReceipt;

type AppState = Arc<ReceiptStore>;

// ── Query params ──────────────────────────────────────────────────────────────

#[derive(Deserialize)]
struct ListQuery {
    agent_id: Option<String>,
    kind:     Option<String>,
    limit:    Option<usize>,
}

// ── Handlers ──────────────────────────────────────────────────────────────────

async fn health() -> impl IntoResponse {
    Json(json!({ "ok": true, "service": "arp-broker", "port": ARP_PORT }))
}

async fn submit_receipt(
    State(store): State<AppState>,
    Json(receipt): Json<ActionReceipt>,
) -> impl IntoResponse {
    let id = receipt.receipt_id.to_string();
    let (chain_result, gix_id) = store.submit(receipt);

    let chain_status = match &chain_result {
        arp_broker::ChainResult::Genesis              => "genesis",
        arp_broker::ChainResult::Valid { .. }         => "valid",
        arp_broker::ChainResult::Broken { .. }        => "broken",
        arp_broker::ChainResult::PrevNotFound { .. }  => "prev_not_found",
    };

    let status_code = if matches!(chain_result, arp_broker::ChainResult::Broken { .. }) {
        StatusCode::UNPROCESSABLE_ENTITY
    } else {
        StatusCode::CREATED
    };

    (status_code, Json(json!({
        "receipt_id":    id,
        "chain_status":  chain_status,
        "chain_result":  format!("{:?}", chain_result),
        "gix_canonical": gix_id,
        "total_receipts": store.len(),
    })))
}

async fn list_receipts(
    State(store): State<AppState>,
    Query(q): Query<ListQuery>,
) -> impl IntoResponse {
    let limit = q.limit.unwrap_or(50).min(500);
    let receipts = store.list(q.agent_id.as_deref(), q.kind.as_deref(), limit);
    Json(json!({ "receipts": receipts, "count": receipts.len() }))
}

async fn get_receipt_handler(
    State(store): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    match store.get(&id) {
        Some(r) => (StatusCode::OK, Json(serde_json::to_value(r).unwrap_or_default())),
        None    => (StatusCode::NOT_FOUND, Json(json!({ "error": "receipt not found" }))),
    }
}

async fn verify_chain(
    State(store): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    match store.get(&id) {
        None => (StatusCode::NOT_FOUND, Json(json!({ "error": "not found" }))),
        Some(receipt) => {
            let hash = receipt.hash();
            let prev = receipt.previous_hash.clone();
            (StatusCode::OK, Json(json!({
                "receipt_id":    id,
                "hash":          hash,
                "previous_hash": prev,
                "is_genesis":    prev.is_none(),
            })))
        }
    }
}

async fn gix_entry(
    State(store): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    match store.gix_entry_for(&id) {
        Some(e) => (StatusCode::OK, Json(serde_json::to_value(e).unwrap_or_default())),
        None    => (StatusCode::NOT_FOUND, Json(json!({ "error": "receipt not in GIX index" }))),
    }
}

async fn audit_root(State(store): State<AppState>) -> impl IntoResponse {
    Json(json!({
        "merkle_root":   store.merkle_root(),
        "total_receipts": store.len(),
    }))
}

// ── Startup ───────────────────────────────────────────────────────────────────

const ARP_PORT: u16 = 7795;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    let port: u16 = std::env::var("ARP_PORT")
        .ok().and_then(|s| s.parse().ok())
        .unwrap_or(ARP_PORT);

    let store: AppState = Arc::new(ReceiptStore::new());

    let app = Router::new()
        .route("/health",                              get(health))
        .route("/api/receipts",                        get(list_receipts).post(submit_receipt))
        .route("/api/receipts/audit",                  get(audit_root))
        .route("/api/receipts/:id",                    get(get_receipt_handler))
        .route("/api/receipts/:id/verify",             post(verify_chain))
        .route("/api/receipts/:id/gix",                get(gix_entry))
        .with_state(store);

    let addr = format!("0.0.0.0:{port}");
    tracing::info!("arp-broker listening on {addr}");
    let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
