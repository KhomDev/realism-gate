//! Replays the fuzz seed corpus through every fuzz target, and runs a fixed set
//! of deterministic mutations of each seed. The property is the same as the
//! cargo-fuzz targets': no target panics.
//!
//! The bodies come from `fuzz/src/lib.rs`, the same file the cargo-fuzz
//! binaries call. cargo-fuzz (libFuzzer) does not run on Windows, and this
//! harness does, so the corpus is exercised on every CI platform. The weekly
//! `fuzz.yml` job is the long run; this is the short one.
//!
//! A failure names the target, the seed and the mutant index. The mutations
//! come from a fixed xorshift sequence, so a failure reproduces exactly.

#[path = "../fuzz/src/lib.rs"]
#[allow(dead_code)]
mod targets;

use std::panic::{self, AssertUnwindSafe};
use std::path::{Path, PathBuf};

/// Mutants per seed, besides the seed itself. Small enough to keep the test
/// fast in a debug build. `REALISM_GATE_FUZZ_MUTANTS=N` raises it for a local
/// search, for example with `--release`; the weekly job runs the real search.
const MUTANTS_PER_SEED: usize = 24;

fn mutants_per_seed() -> usize {
    std::env::var("REALISM_GATE_FUZZ_MUTANTS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(MUTANTS_PER_SEED)
}

/// Byte strings the parsers treat as syntax. Inserting them is more likely to
/// reach a new branch than random bytes are.
const TOKENS: &[&[u8]] = &[
    b"{",
    b"}",
    b"(",
    b")",
    b"[",
    b"]",
    b"\"",
    b"'",
    b"/*",
    b"*/",
    b"//",
    b"\n",
    b"\r\n",
    b"\0",
    b"\xff",
    b"<",
    b">",
    b"::",
    b"#[test]",
    b"---",
    b"  - ",
    b":",
    b"vm.prank(",
    b"\\u{",
    b"`",
    b"${",
    b"0x",
    b"fn ",
    b"contract ",
    b"module ",
    b"\xe2\x80\x8b",
];

/// xorshift64: a fixed sequence for a fixed seed.
struct Rng(u64);

impl Rng {
    fn new(seed: u64) -> Rng {
        Rng(seed | 1)
    }

    fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }

    fn below(&mut self, n: usize) -> usize {
        (self.next() % n.max(1) as u64) as usize
    }
}

/// One mutation of `seed`: truncate, delete a span, insert a token, overwrite
/// a byte, or repeat a span.
fn mutate(seed: &[u8], rng: &mut Rng) -> Vec<u8> {
    let mut out = seed.to_vec();
    match rng.below(5) {
        0 => {
            let n = rng.below(out.len() + 1);
            out.truncate(n);
        }
        1 if !out.is_empty() => {
            let a = rng.below(out.len());
            let b = (a + 1 + rng.below(64)).min(out.len());
            out.drain(a..b);
        }
        2 => {
            let at = rng.below(out.len() + 1);
            let token = TOKENS[rng.below(TOKENS.len())];
            out.splice(at..at, token.iter().copied());
        }
        3 if !out.is_empty() => {
            let i = rng.below(out.len());
            out[i] = rng.next() as u8;
        }
        4 if !out.is_empty() => {
            let a = rng.below(out.len());
            let b = (a + rng.below(128)).min(out.len());
            let span = out[a..b].to_vec();
            let at = rng.below(out.len() + 1);
            out.splice(at..at, span);
        }
        _ => {}
    }
    out
}

/// The seeds for one target, in name order, so the run is the same everywhere.
fn seeds(target: &str) -> Vec<(PathBuf, Vec<u8>)> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("fuzz/corpus")
        .join(target);
    let mut paths: Vec<PathBuf> = std::fs::read_dir(&dir)
        .unwrap_or_else(|e| panic!("corpus for {target} at {}: {e}", dir.display()))
        .map(|entry| entry.unwrap().path())
        .filter(|p| p.is_file())
        .collect();
    paths.sort();
    paths
        .into_iter()
        .map(|p| {
            let bytes = std::fs::read(&p).unwrap();
            (p, bytes)
        })
        .collect()
}

/// A stable 64-bit hash of a name, so each seed has its own mutation sequence.
fn fnv(name: &str) -> u64 {
    name.bytes().fold(0xcbf2_9ce4_8422_2325, |h, b| {
        (h ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3)
    })
}

#[test]
fn the_seed_corpus_has_every_target() {
    for (name, _) in targets::TARGETS {
        assert!(!seeds(name).is_empty(), "no seeds for {name}");
    }
}

#[test]
fn no_target_panics_on_a_seed_or_its_mutants() {
    let mutants = mutants_per_seed();
    let mut failures = Vec::new();
    let mut runs = 0usize;
    for (name, run) in targets::TARGETS {
        for (path, seed) in seeds(name) {
            let mut rng = Rng::new(fnv(&path.file_name().unwrap().to_string_lossy()));
            let mut inputs = vec![seed.clone()];
            inputs.extend((0..mutants).map(|_| mutate(&seed, &mut rng)));
            for (index, input) in inputs.iter().enumerate() {
                runs += 1;
                if panic::catch_unwind(AssertUnwindSafe(|| run(input))).is_err() {
                    let label = if index == 0 {
                        "the seed itself".to_string()
                    } else {
                        format!("mutant {index}")
                    };
                    failures.push(format!("{name}: {} ({label})", path.display()));
                }
            }
        }
    }
    assert!(
        failures.is_empty(),
        "{} panic(s) in {runs} runs:\n{}",
        failures.len(),
        failures.join("\n")
    );
}
