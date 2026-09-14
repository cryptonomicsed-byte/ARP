use crate::receipt::ActionReceipt;

fn zangbeto_base() -> String {
    std::env::var("ZANGBETO_URL")
        .or_else(|_| std::env::var("VANTAGE_API_URL"))
        .unwrap_or_else(|_| "http://127.0.0.1:8000".to_string())
}

fn api_key() -> String {
    std::env::var("VANTAGE_API_KEY").unwrap_or_default()
}

/// Submit an ActionReceipt to Zàngbétò for canonical settlement.
/// Returns the anchor_id (Zàngbétò record hash) on success.
/// Fail-open: receipt operations are never blocked by Zàngbétò being unreachable.
pub async fn anchor_receipt(receipt: &ActionReceipt) -> Option<String> {
    let client = reqwest::Client::new();
    let key = api_key();

    let body = serde_json::json!({
        "receipt_id":     receipt.receipt_id,
        "kind":           receipt.kind,
        "principal_id":   receipt.principal.principal_id,
        "agent_id":       receipt.principal.agent_id,
        "action_kind":    receipt.action.kind,
        "action_target":  receipt.action.target,
        "outcome":        receipt.action.outcome,
        "canonical_hash": receipt.hash(),
        "timestamp":      receipt.timestamp,
        "previous_hash":  receipt.previous_hash,
    });

    let url = format!("{}/api/zangbeto/records", zangbeto_base());
    let mut req = client.post(&url).json(&body)
        .timeout(std::time::Duration::from_secs(8));
    if !key.is_empty() {
        req = req.header("X-Agent-Key", &key);
    }

    match req.send().await {
        Ok(resp) if resp.status().is_success() => {
            resp.json::<serde_json::Value>().await.ok()
                .and_then(|v| v.get("anchor_id").and_then(|a| a.as_str()).map(String::from))
        }
        _ => None,
    }
}

/// Verify a receipt against its Zàngbétò anchor.
/// Returns true if the anchor exists and the canonical_hash matches.
pub async fn verify_anchor(receipt_id: &str, canonical_hash: &str) -> bool {
    let client = reqwest::Client::new();
    let url = format!("{}/api/zangbeto/records/{}", zangbeto_base(), receipt_id);
    let key = api_key();
    let mut req = client.get(&url).timeout(std::time::Duration::from_secs(5));
    if !key.is_empty() {
        req = req.header("X-Agent-Key", &key);
    }

    match req.send().await {
        Ok(resp) if resp.status().is_success() => {
            resp.json::<serde_json::Value>().await.ok()
                .and_then(|v| v.get("canonical_hash").and_then(|h| h.as_str()).map(String::from))
                .map(|stored| stored == canonical_hash)
                .unwrap_or(false)
        }
        _ => false,
    }
}
