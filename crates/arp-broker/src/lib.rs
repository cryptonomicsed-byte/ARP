//! arp-broker — Action Receipt Protocol v1 HTTP broker.
//!
//! Routes:
//!   POST /api/receipts               — submit an ActionReceipt
//!   GET  /api/receipts               — list receipts (optional ?agent_id=&kind=&limit=)
//!   GET  /api/receipts/:id           — get a single receipt
//!   POST /api/receipts/:id/verify    — verify chain continuity to the previous receipt
//!   GET  /api/receipts/:id/gix       — get the GIX1 entry for a receipt
//!   GET  /api/receipts/audit         — Merkle root over all stored receipts
//!   GET  /health                     — liveness probe

pub mod store;
pub use store::{ReceiptStore, ChainResult};
