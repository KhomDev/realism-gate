//! The fuzz targets' bodies. Each takes raw bytes and must never panic.
//!
//! This file has two users:
//! - the cargo-fuzz binaries in `fuzz/fuzz_targets/` (libFuzzer, nightly, Linux);
//! - `tests/fuzz_corpus.rs` in the main crate, which is compiled with
//!   `#[path]`, replays the seed corpus under `fuzz/corpus/` through the same
//!   functions on every `cargo test`, and runs deterministic mutations of it.
//!
//! So nothing here may depend on libFuzzer. Input that is not UTF-8 is lossily
//! decoded: the parsers take text, and the bytes that do not decode are still
//! input worth trying.

use realism_gate::claims::Claims;
use realism_gate::declaration::Declaration;

fn text(data: &[u8]) -> String {
    String::from_utf8_lossy(data).into_owned()
}

/// Solidity: the `solar` frontend that lowers Foundry tests.
pub fn solidity(data: &[u8]) {
    let _ = realism_gate::parse::parse_source("fuzz.t.sol", &text(data));
}

/// Rust (Anchor and native Solana tests): the `syn` frontend.
pub fn rust(data: &[u8]) {
    let _ = realism_gate::solana::parse_rust::parse_source("fuzz.rs", &text(data));
}

/// TypeScript (Anchor tests): the `oxc` frontend.
pub fn typescript(data: &[u8]) {
    let _ = realism_gate::solana::parse_ts::parse_source("fuzz.ts", &text(data));
}

/// Move: the reader for `#[test]` functions, signers and calls.
pub fn move_lang(data: &[u8]) {
    let _ = realism_gate::move_lang::parse_source(&text(data));
}

/// Claims: `claims.yaml`, front matter of a finding, and the prose heuristics
/// that run when neither is present.
pub fn claims(data: &[u8]) {
    let t = text(data);
    let _ = Claims::parse(&t);
    if let Some(block) = realism_gate::claims::front_matter(&t) {
        let _ = Claims::parse(block);
    }
    let _ = realism_gate::claims::extract::inferred(&t);
}

/// `realism.yaml`: the facts a reviewer states about the repository.
pub fn declaration(data: &[u8]) {
    let _ = Declaration::parse(&text(data));
}

/// One fuzz body: takes raw bytes, must not panic.
pub type Target = fn(&[u8]);

/// Every target, by name. The name is also the corpus directory under
/// `fuzz/corpus/`.
pub const TARGETS: &[(&str, Target)] = &[
    ("solidity", solidity),
    ("rust", rust),
    ("typescript", typescript),
    ("move", move_lang),
    ("claims", claims),
    ("declaration", declaration),
];
