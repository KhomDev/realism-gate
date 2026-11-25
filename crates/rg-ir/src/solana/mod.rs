//! The Solana test model.
//!
//! [`ir`] is the owned model of one Solana test file, Rust or TypeScript. Both
//! frontends lower into it, so the trace, the identities and the rules do not
//! care which language a test is written in.

pub mod ir;
