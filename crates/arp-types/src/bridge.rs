//! ArpBridge — wraps ActionReceipts as GIX1 entries.
//!
//! This is the first production consumer of gix-types in ARP.
//! Every ActionReceipt that passes through the bridge gets a content-addressed
//! GlyphNode (kind=Receipt) inserted into the caller's Gix1Index, making all
//! sovereign receipts queryable via the GIX graph.

use gix_types::{Gix1Entry, GixKind, gix1_merkle_root, gix1_audit};
use crate::receipt::ActionReceipt;

/// Bridge between ARP receipts and the GIX1 index.
///
/// Maintains an in-memory list of `Gix1Entry` records derived from `ActionReceipt`s.
/// Call `merkle_root()` to get the current Merkle root, `audit(stored)` to verify.
#[derive(Debug, Default, Clone)]
pub struct ArpBridge {
    entries: Vec<Gix1Entry>,
}

impl ArpBridge {
    pub fn new() -> Self {
        Self::default()
    }

    /// Wrap an ActionReceipt as a Gix1Entry with `GixKind::Receipt`.
    ///
    /// Uses `receipt_id` as the canonical identity string.
    /// `ts` is the receipt timestamp (Unix seconds as f64).
    pub fn ingest(&mut self, receipt: &ActionReceipt) -> &Gix1Entry {
        let entry = Gix1Entry::from_receipt(
            &receipt.receipt_id.to_string(),
            GixKind::Receipt,
            receipt.timestamp as f64,
        );
        self.entries.push(entry);
        self.entries.last().unwrap()
    }

    /// Ingest with an explicit GixKind override (e.g. Simulation, Governance).
    pub fn ingest_as(&mut self, receipt: &ActionReceipt, kind: GixKind) -> &Gix1Entry {
        let entry = Gix1Entry::from_receipt(
            &receipt.receipt_id.to_string(),
            kind,
            receipt.timestamp as f64,
        );
        self.entries.push(entry);
        self.entries.last().unwrap()
    }

    /// All ingested entries.
    pub fn entries(&self) -> &[Gix1Entry] {
        &self.entries
    }

    /// GIX1 Merkle root over all ingested receipt canonical_ids.
    pub fn merkle_root(&self) -> String {
        let ids: Vec<&str> = self.entries.iter()
            .map(|e| e.canonical_id.as_str())
            .collect();
        gix1_merkle_root(&ids)
    }

    /// Verify a stored Merkle root against the current entry set.
    pub fn audit(&self, stored_root: &str) -> Result<String, String> {
        let ids: Vec<&str> = self.entries.iter()
            .map(|e| e.canonical_id.as_str())
            .collect();
        gix1_audit(stored_root, &ids)
    }

    /// Find the Gix1Entry for a specific receipt_id (if ingested).
    pub fn find(&self, receipt_id: &str) -> Option<&Gix1Entry> {
        let target = gix_types::content_hash(receipt_id);
        let target_hex = hex::encode(target);
        self.entries.iter().find(|e| e.canonical_id == target_hex)
    }

    /// Number of ingested entries.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;
    use crate::receipt::{ActionReceipt, ActionSpec, ReceiptKind};
    use crate::principal::{Principal, PrincipalKind, CapabilityRef};

    fn dummy_receipt() -> ActionReceipt {
        ActionReceipt {
            receipt_id: Uuid::new_v4(),
            kind: ReceiptKind::Compute,
            kind_ext: None,
            principal: Principal {
                principal_id: Uuid::new_v4().to_string(),
                kind: PrincipalKind::Agent,
                agent_id: "agent-test".into(),
                capabilities: vec![CapabilityRef { capability: "compute.submit".into(), required_tier: None, grant_id: None }],
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
            signature: "mocksig".into(),
            gix1_canonical_id: None,
        }
    }

    #[test]
    fn ingest_and_root() {
        let mut bridge = ArpBridge::new();
        let r1 = dummy_receipt();
        let r2 = dummy_receipt();
        bridge.ingest(&r1);
        bridge.ingest(&r2);
        assert_eq!(bridge.len(), 2);
        let root = bridge.merkle_root();
        assert_eq!(root.len(), 64);
        assert!(bridge.audit(&root).is_ok());
        assert!(bridge.audit("badhash").is_err());
    }

    #[test]
    fn find_by_receipt_id() {
        let mut bridge = ArpBridge::new();
        let r = dummy_receipt();
        let id_str = r.receipt_id.to_string();
        bridge.ingest(&r);
        assert!(bridge.find(&id_str).is_some());
        assert!(bridge.find("nonexistent").is_none());
    }

    #[test]
    fn kind_override() {
        let mut bridge = ArpBridge::new();
        let r = dummy_receipt();
        let entry = bridge.ingest_as(&r, GixKind::Simulation).clone();
        assert!(matches!(entry.kind, GixKind::Simulation));
    }
}
