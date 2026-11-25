#![forbid(unsafe_code)]
//! The RG01-RG20 rules and the language-specific fact extraction they need.
//!
//! Each rule is a way a passing PoC can prove less than it claims, and that
//! does not depend on the language. The Solidity rules read the trace and the
//! identities directly ([`rules`]); the Solana and Move rules apply the same
//! ids to their own models ([`solana::rules`], [`move_lang::rules`]).
//!
//! The modules are re-exported so that `crate::ir`, `crate::identity`,
//! `crate::project` and `crate::trace` resolve the way they did when this was
//! one crate, and so the binary crate can keep `realism_gate::rules`,
//! `realism_gate::solana` and `realism_gate::move_lang` unchanged.

pub mod move_lang;
pub mod rules;
pub mod solana;

pub use rg_ir::{finding, ir};
pub use rg_model::{identity, project, trace};
