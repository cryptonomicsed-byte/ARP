//! In-memory receipt store with hash-chain verification and GIX1 bridge.

use std::collections::HashMap;
use std::sync::RwLock;

use arp_types::{ActionReceipt, ArpBridge};

#[derive(Debug, Clone)]
pub enum ChainResult {
    /// Genesis receipt (no predecessor).
    Genesis,
    /// Chain is valid: this receipt's previous_hash == pred.hash().
    Valid { prev_id: String },
    /// Chain is broken: hashes don't match.
    Broken { expected: String, got: String },
    /// Previous receipt not found in store.
    PrevNotFound { prev_hash: String },
}

pub struct ReceiptStore {
    /// receipt_id → ActionReceipt
    receipts: RwLock<HashMap<String, ActionReceipt>>,
    /// per-agent: latest receipt_id (for chain tracking)
    heads:    RwLock<HashMap<String, String>>,
    /// GIX1 bridge — all ingested receipts
    bridge:   RwLock<ArpBridge>,
}

impl ReceiptStore {
    pub fn new() -> Self {
        Self {
            receipts: RwLock::new(HashMap::new()),
            heads:    RwLock::new(HashMap::new()),
            bridge:   RwLock::new(ArpBridge::new()),
        }
    }

    /// Submit a receipt.  Returns (ChainResult, gix1_canonical_id).
    pub fn submit(&self, receipt: ActionReceipt) -> (ChainResult, String) {
        let mut receipt = receipt;
        let id      = receipt.receipt_id.to_string();
        let agent   = receipt.principal.agent_id.clone();

        // Chain verification
        let chain_result = match &receipt.previous_hash {
            None => ChainResult::Genesis,
            Some(expected_hash) => {
                let receipts = self.receipts.read().unwrap();
                // Find a receipt whose hash() == expected_hash
                let pred = receipts.values().find(|r| &r.hash() == expected_hash);
                match pred {
                    None => ChainResult::PrevNotFound { prev_hash: expected_hash.clone() },
                    Some(p) => {
                        if receipt.chain_valid(p) {
                            ChainResult::Valid { prev_id: p.receipt_id.to_string() }
                        } else {
                            ChainResult::Broken {
                                expected: expected_hash.clone(),
                                got:      p.hash(),
                            }
                        }
                    }
                }
            }
        };

        // Ingest into GIX1 bridge
        let gix_id = {
            let mut bridge = self.bridge.write().unwrap();
            bridge.ingest(&receipt).canonical_id.clone()
        };
        receipt.gix1_canonical_id = Some(gix_id.clone());

        // Store receipt and update head
        {
            let mut map = self.receipts.write().unwrap();
            map.insert(id.clone(), receipt);
        }
        {
            let mut heads = self.heads.write().unwrap();
            heads.insert(agent, id);
        }

        (chain_result, gix_id)
    }

    pub fn get(&self, receipt_id: &str) -> Option<ActionReceipt> {
        self.receipts.read().unwrap().get(receipt_id).cloned()
    }

    pub fn list(
        &self,
        agent_id: Option<&str>,
        kind: Option<&str>,
        limit: usize,
    ) -> Vec<ActionReceipt> {
        let map = self.receipts.read().unwrap();
        let mut out: Vec<ActionReceipt> = map.values()
            .filter(|r| {
                if let Some(a) = agent_id { if r.principal.agent_id != a { return false; } }
                if let Some(k) = kind {
                    let kstr = format!("{:?}", r.kind).to_lowercase();
                    if !kstr.contains(&k.to_lowercase()) { return false; }
                }
                true
            })
            .cloned()
            .collect();
        out.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
        out.truncate(limit);
        out
    }

    /// Merkle root over all stored receipts (via GIX1 bridge).
    pub fn merkle_root(&self) -> String {
        self.bridge.read().unwrap().merkle_root()
    }

    pub fn gix_entry_for(&self, receipt_id: &str) -> Option<gix_types::Gix1Entry> {
        self.bridge.read().unwrap().find(receipt_id).cloned()
    }

    pub fn len(&self) -> usize {
        self.receipts.read().unwrap().len()
    }
}

impl Default for ReceiptStore {
    fn default() -> Self { Self::new() }
}

#[cfg(test)]
mod gix_tests {
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
    fn submit_stamps_gix1_on_receipt() {
        let store = ReceiptStore::new();
        let r = dummy_receipt();
        let id = r.receipt_id.to_string();
        let (_, gix_id) = store.submit(r);
        assert_eq!(gix_id.len(), 64);
        let stored = store.get(&id).unwrap();
        assert_eq!(stored.gix1_canonical_id, Some(gix_id));
    }

    #[test]
    fn two_receipts_have_distinct_gix1_ids() {
        let store = ReceiptStore::new();
        let (_, id1) = store.submit(dummy_receipt());
        let (_, id2) = store.submit(dummy_receipt());
        assert_ne!(id1, id2);
    }
}
