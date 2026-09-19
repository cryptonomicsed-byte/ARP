# ARP — Action Receipt Protocol

Unifies 6 incompatible receipt formats across the sovereign ecosystem into one canonical envelope. Every economic and operational action produces an ARP receipt.

**Port:** none (library) | **Security:** high | **ARP:** is the receipt format

## 5 Primitives (chain order)

```
Principal → Capability → Action → Evidence → Receipt
```

| Primitive | Description |
|-----------|-------------|
| `Principal` | Who acted (agent npub + role) |
| `Capability` | What they were authorized to do |
| `Action` | The specific operation performed |
| `Evidence` | Proof of outcome (hash, score, log ref) |
| `Receipt` | Signed envelope linking all 4 above |

## Receipt Kinds

`compute` · `vcp_session` · `emission` · `twin_capture` · `twin_scene` · `simulation` · `governance` · `economic` · `agent_lifecycle` · `witness` · `mesh_event` · `custom`

## Architecture

```
ARP workspace
├── crates/arp-types/       # ARP envelope types, receipt kinds
├── crates/arp-core/        # signing, verification, chain linkage
└── MANIFEST.toml
```

## Chain Linkage

Each receipt includes `previous_hash` (sha256) forming an append-only chain per principal. Orphaned receipts use `previous_hash = "genesis"`.

## Usage

```rust
use arp_types::{ArpReceipt, ArpPrincipal, ArpAction, ArpEvidence};

let receipt = ArpReceipt::build()
    .principal(principal)
    .capability(cap)
    .action(action)
    .evidence(evidence)
    .sign(&keypair)?;
```

## Integrations

Used by: Vantage (action_receipt.py), UCX (compute jobs), VCP (device sessions), ScarabSwarm (sim receipts), Witness (attestations), OSOVM (emission), all economic flows.
