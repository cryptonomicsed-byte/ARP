//! Ed25519 receipt signing for arp-broker.
//!
//! Reads `BROKER_SIGNING_KEY` (32-byte hex seed) from the environment.
//! If absent, logs a warning and leaves the signature field unchanged (fail-open).

use arp_types::ActionReceipt;
use ed25519_dalek::{SigningKey, Signer};

/// Sign `receipt.hash()` with the broker's Ed25519 key.
///
/// Sets `receipt.signature` to the lowercase hex-encoded 64-byte signature.
/// If `BROKER_SIGNING_KEY` is not set or is malformed, logs a warning and returns
/// without modifying the receipt.
pub fn sign_receipt(receipt: &mut ActionReceipt) {
    let key_hex = match std::env::var("BROKER_SIGNING_KEY") {
        Ok(k) => k,
        Err(_) => {
            tracing::warn!("BROKER_SIGNING_KEY not set — skipping receipt signing");
            return;
        }
    };

    match derive_signing_key(&key_hex) {
        Ok(signing_key) => {
            let message = receipt.hash();
            let sig = signing_key.sign(message.as_bytes());
            receipt.signature = hex::encode(sig.to_bytes());
        }
        Err(e) => {
            tracing::warn!("BROKER_SIGNING_KEY invalid — skipping receipt signing: {e}");
        }
    }
}

fn derive_signing_key(key_hex: &str) -> Result<SigningKey, String> {
    let bytes = hex::decode(key_hex).map_err(|e| format!("hex decode: {e}"))?;
    let arr: [u8; 32] = bytes
        .try_into()
        .map_err(|_| "BROKER_SIGNING_KEY must be exactly 32 bytes (64 hex chars)".to_string())?;
    Ok(SigningKey::from_bytes(&arr))
}

#[cfg(test)]
mod tests {
    use super::*;
    use arp_types::receipt::{ActionReceipt, ActionSpec, ReceiptKind};
    use arp_types::principal::{Principal, PrincipalKind, CapabilityRef};
    use uuid::Uuid;

    fn dummy_receipt() -> ActionReceipt {
        ActionReceipt {
            receipt_id: Uuid::new_v4(),
            kind: ReceiptKind::Compute,
            kind_ext: None,
            principal: Principal {
                principal_id: Uuid::new_v4().to_string(),
                kind: PrincipalKind::Agent,
                agent_id: "agent-test".into(),
                capabilities: vec![CapabilityRef {
                    capability: "compute.submit".into(),
                    required_tier: None,
                    grant_id: None,
                }],
                session_id: None,
                agent_tier: None,
            },
            action: ActionSpec {
                kind: "submit_job".into(),
                target: "job-abc".into(),
                outcome: "success".into(),
                params: serde_json::json!({}),
            },
            evidence_ids: vec![],
            witness_attestations: vec![],
            throne_evaluations: vec![],
            consensus_receipt: None,
            physical_attestation: None,
            zangbeto_anchor: None,
            nostr_event_id: None,
            timestamp: 1_700_000_000,
            execution_id: None,
            previous_hash: None,
            signature: String::new(),
            gix1_canonical_id: None,
        }
    }

    #[test]
    fn sign_receipt_sets_signature_when_key_set() {
        // Use a known 32-byte seed
        let seed = [42u8; 32];
        let key_hex = hex::encode(seed);
        std::env::set_var("BROKER_SIGNING_KEY", &key_hex);

        let mut receipt = dummy_receipt();
        sign_receipt(&mut receipt);

        assert!(!receipt.signature.is_empty(), "signature should be set");
        assert_eq!(receipt.signature.len(), 128, "Ed25519 sig = 64 bytes = 128 hex chars");

        std::env::remove_var("BROKER_SIGNING_KEY");
    }

    #[test]
    fn sign_receipt_is_stable_for_same_key_and_receipt() {
        let seed = [7u8; 32];
        let key_hex = hex::encode(seed);
        std::env::set_var("BROKER_SIGNING_KEY", &key_hex);

        let mut r1 = dummy_receipt();
        let mut r2 = r1.clone();
        sign_receipt(&mut r1);
        sign_receipt(&mut r2);

        assert_eq!(r1.signature, r2.signature);
        std::env::remove_var("BROKER_SIGNING_KEY");
    }

    #[test]
    fn sign_receipt_leaves_empty_when_no_key() {
        std::env::remove_var("BROKER_SIGNING_KEY");
        let mut receipt = dummy_receipt();
        sign_receipt(&mut receipt);
        assert!(receipt.signature.is_empty());
    }
}
