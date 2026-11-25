//! The Solana model: the environment, the harness reader and the trace.
//!
//! The parsers live in `rg-frontend-rust` and `rg-frontend-ts`; this module
//! re-exports them under the paths the pipeline used before the workspace
//! split, together with the shared Solana IR from `rg-ir`.

pub mod env;
pub mod harness;
pub mod trace;

pub use rg_frontend_rust::parse_rust;
pub use rg_frontend_ts::parse_ts;
pub use rg_ir::solana::ir;
