//! The fixture matrix: every synthetic program resolves to exactly the
//! authorities its source demands, at the file and line they are written.
//!
//! Each case names a source line by a marker rather than a hard-coded number,
//! so a fixture edit cannot silently move an expected span; the marker's line
//! is looked up in the fixture at test time and compared exactly.

use rg_program::{Authority, EntryPointKind, Language, ProgramModel, Provenance};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

#[derive(Clone)]
struct Expected {
    kind: &'static str,
    name: Option<&'static str>,
    file: &'static str,
    line: u32,
    provenance: &'static str,
}

struct Case {
    fixture: &'static str,
    program: &'static str,
    entry: &'static str,
    kind: EntryPointKind,
    expected: Vec<Expected>,
}

impl Case {
    fn new(
        fixture: &'static str,
        program: &'static str,
        entry: &'static str,
        kind: EntryPointKind,
    ) -> Self {
        Self {
            fixture,
            program,
            entry,
            kind,
            expected: Vec::new(),
        }
    }

    fn auth(
        mut self,
        kind: &'static str,
        name: Option<&'static str>,
        file: &'static str,
        marker: &'static str,
        provenance: &'static str,
    ) -> Self {
        let line = marker_line(self.fixture, file, marker);
        self.expected.push(Expected {
            kind,
            name,
            file,
            line,
            provenance,
        });
        self
    }

    fn auth_line(
        mut self,
        kind: &'static str,
        name: Option<&'static str>,
        file: &'static str,
        line: u32,
        provenance: &'static str,
    ) -> Self {
        self.expected.push(Expected {
            kind,
            name,
            file,
            line,
            provenance,
        });
        self
    }
}

fn fixtures_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

fn fixture_path(name: &str) -> PathBuf {
    fixtures_root().join(name)
}

fn marker_line(fixture: &str, file: &str, marker: &str) -> u32 {
    let path = fixture_path(fixture).join(file);
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("{}: {error}", path.display()));
    let found: Vec<u32> = text
        .lines()
        .enumerate()
        .filter(|(_, line)| line.contains(marker))
        .map(|(index, _)| u32::try_from(index + 1).unwrap())
        .collect();
    assert_eq!(
        found.len(),
        1,
        "marker {marker:?} must occur exactly once in {fixture}/{file}, found on lines {found:?}"
    );
    found[0]
}

fn provenance_name(provenance: Provenance) -> &'static str {
    match provenance {
        Provenance::Idl => "idl",
        Provenance::AnchorStruct => "anchor-struct",
        Provenance::NativeCheck => "native-check",
        Provenance::SolidityModifier => "solidity-modifier",
        Provenance::SolidityBody => "solidity-body",
    }
}

fn authority_rows(
    entry: &rg_program::EntryPoint,
) -> BTreeSet<(String, Option<String>, String, u32, String)> {
    let mut rows = BTreeSet::new();
    for authority in &entry.authorities {
        for span in authority.spans() {
            rows.insert((
                authority.kind().to_string(),
                authority.name().map(ToString::to_string),
                span.file.clone(),
                span.line,
                provenance_name(authority.provenance()).to_string(),
            ));
        }
    }
    rows
}

fn cases() -> Vec<Case> {
    use EntryPointKind::{Function, Instruction};
    vec![
        // --- Anchor -------------------------------------------------------
        Case::new("anchor-signer-direct", "vault", "withdraw", Instruction).auth(
            "signer",
            Some("authority"),
            "src/lib.rs",
            "pub authority: Signer<'info>,",
            "anchor-struct",
        ),
        Case::new("anchor-signer-optional", "vault", "open", Instruction).auth(
            "none",
            None,
            "src/lib.rs",
            "pub fn open",
            "anchor-struct",
        ),
        Case::new("anchor-signer-boxed", "vault", "fund", Instruction).auth(
            "signer",
            Some("payer"),
            "src/lib.rs",
            "pub payer: Box<Signer<'info>>,",
            "anchor-struct",
        ),
        Case::new("anchor-signer-negative", "vault", "peek", Instruction).auth(
            "none",
            None,
            "src/lib.rs",
            "pub fn peek",
            "anchor-struct",
        ),
        Case::new("anchor-has-one", "vault", "withdraw", Instruction)
            .auth(
                "signer",
                Some("authority"),
                "src/lib.rs",
                "pub authority: Signer<'info>,",
                "anchor-struct",
            )
            .auth(
                "owner",
                None,
                "src/lib.rs",
                "has_one = authority",
                "anchor-struct",
            ),
        Case::new("anchor-error-suffix", "vault", "settle", Instruction)
            .auth(
                "signer",
                Some("admin"),
                "src/lib.rs",
                "pub admin: Signer<'info>,",
                "anchor-struct",
            )
            .auth(
                "owner",
                None,
                "src/lib.rs",
                "has_one = admin @",
                "anchor-struct",
            ),
        Case::new(
            "anchor-duplicate-across-crates",
            "alpha",
            "open",
            Instruction,
        )
        .auth(
            "signer",
            Some("authority"),
            "programs/alpha/src/lib.rs",
            "pub authority: Signer<'info>,",
            "anchor-struct",
        ),
        Case::new(
            "anchor-duplicate-across-crates",
            "beta",
            "open",
            Instruction,
        )
        .auth(
            "signer",
            Some("payer"),
            "programs/beta/src/lib.rs",
            "pub payer: Signer<'info>,",
            "anchor-struct",
        ),
        Case::new("anchor-has-one-chain", "vault", "settle", Instruction)
            .auth(
                "signer",
                Some("authority"),
                "src/lib.rs",
                "pub authority: Signer<'info>,",
                "anchor-struct",
            )
            .auth(
                "signer",
                Some("operator"),
                "src/lib.rs",
                "pub operator: Signer<'info>,",
                "anchor-struct",
            )
            .auth(
                "owner",
                None,
                "src/lib.rs",
                "has_one = authority,",
                "anchor-struct",
            )
            .auth(
                "owner",
                None,
                "src/lib.rs",
                "has_one = operator",
                "anchor-struct",
            ),
        Case::new(
            "anchor-constraint-keyeq-signer",
            "vault",
            "withdraw",
            Instruction,
        )
        .auth(
            "signer",
            Some("authority"),
            "src/lib.rs",
            "pub authority: Signer<'info>,",
            "anchor-struct",
        )
        .auth(
            "role",
            Some("config.authority"),
            "src/lib.rs",
            "constraint = config.authority == authority.key()",
            "anchor-struct",
        ),
        Case::new(
            "anchor-constraint-keyeq-nonsigner",
            "vault",
            "close",
            Instruction,
        )
        .auth("none", None, "src/lib.rs", "pub fn close", "anchor-struct"),
        Case::new("anchor-pda", "vault", "initialize", Instruction).auth(
            "signer",
            Some("user"),
            "src/lib.rs",
            "pub user: Signer<'info>,",
            "anchor-struct",
        ),
        Case::new("anchor-pda-no-signer", "vault", "init", Instruction).auth(
            "none",
            None,
            "src/lib.rs",
            "pub fn init",
            "anchor-struct",
        ),
        Case::new("anchor-owner-attr", "vault", "init", Instruction).auth(
            "owner",
            None,
            "src/lib.rs",
            "owner = token_program.key()",
            "anchor-struct",
        ),
        Case::new("anchor-address-attr", "vault", "init", Instruction).auth(
            "none",
            None,
            "src/lib.rs",
            "pub fn init",
            "anchor-struct",
        ),
        Case::new("anchor-address-signer-bound", "vault", "init", Instruction).auth(
            "signer",
            Some("authority"),
            "src/lib.rs",
            "pub authority: Signer<'info>,",
            "anchor-struct",
        ),
        Case::new("anchor-missing-struct", "vault", "open", Instruction).auth(
            "unknown",
            None,
            "src/lib.rs",
            "pub fn open",
            "anchor-struct",
        ),
        Case::new("anchor-duplicate-struct", "vault", "open", Instruction).auth(
            "unknown",
            None,
            "src/program.rs",
            "pub fn open(ctx: Context<Vault>)",
            "anchor-struct",
        ),
        Case::new("anchor-idl-conflict", "vault", "withdraw", Instruction).auth(
            "signer",
            Some("authority"),
            "src/lib.rs",
            "pub authority: Signer<'info>,",
            "anchor-struct",
        ),
        Case::new("anchor-idl-fill", "vault", "open", Instruction).auth_line(
            "signer",
            Some("admin"),
            "target/idl/vault.json",
            1,
            "idl",
        ),
        Case::new("anchor-nested-accounts", "vault", "withdraw", Instruction).auth(
            "signer",
            Some("vault.authority"),
            "src/lib.rs",
            "pub authority: Signer<'info>,",
            "anchor-struct",
        ),
        Case::new("anchor-multi-instruction", "vault", "withdraw", Instruction).auth(
            "signer",
            Some("authority"),
            "src/lib.rs",
            "pub authority: Signer<'info>,",
            "anchor-struct",
        ),
        Case::new("anchor-multi-instruction", "vault", "poke", Instruction).auth(
            "none",
            None,
            "src/lib.rs",
            "pub fn poke(ctx: Context<Poke>)",
            "anchor-struct",
        ),
        Case::new(
            "anchor-unparsable-accounts",
            "vault",
            "withdraw",
            Instruction,
        )
        .auth(
            "unknown",
            None,
            "src/lib.rs",
            "pub fn withdraw",
            "anchor-struct",
        ),
        Case::new("anchor-instruction-no-context", "vault", "bad", Instruction).auth(
            "unknown",
            None,
            "src/lib.rs",
            "pub fn bad(amount: u64)",
            "anchor-struct",
        ),
        // --- Native Solana ------------------------------------------------
        Case::new(
            "native-signer-require",
            "vault",
            "process_withdraw",
            Instruction,
        )
        .auth(
            "signer",
            Some("authority"),
            "programs/vault/src/lib.rs",
            "require!(authority.is_signer",
            "native-check",
        ),
        Case::new(
            "native-signer-if-negated",
            "vault",
            "process_close",
            Instruction,
        )
        .auth(
            "signer",
            Some("authority"),
            "programs/vault/src/lib.rs",
            "if !authority.is_signer",
            "native-check",
        ),
        Case::new(
            "native-signer-conditional",
            "vault",
            "process_peek",
            Instruction,
        )
        .auth(
            "none",
            None,
            "programs/vault/src/lib.rs",
            "pub fn process_peek",
            "native-check",
        ),
        Case::new("native-owner-check", "vault", "process_close", Instruction).auth(
            "none",
            None,
            "programs/vault/src/lib.rs",
            "pub fn process_close",
            "native-check",
        ),
        Case::new(
            "native-key-binds-signer",
            "vault",
            "process_withdraw",
            Instruction,
        )
        .auth(
            "signer",
            Some("authority"),
            "programs/vault/src/lib.rs",
            "require!(authority.is_signer",
            "native-check",
        )
        .auth(
            "role",
            Some("config.authority"),
            "programs/vault/src/lib.rs",
            "if authority.key != config.authority",
            "native-check",
        ),
        Case::new(
            "native-disjunctive",
            "vault",
            "process_withdraw",
            Instruction,
        )
        .auth(
            "unknown",
            None,
            "programs/vault/src/lib.rs",
            "pub fn process_withdraw",
            "native-check",
        ),
        Case::new("native-unnameable", "vault", "process_close", Instruction).auth(
            "unknown",
            None,
            "programs/vault/src/lib.rs",
            "pub fn process_close",
            "native-check",
        ),
        Case::new(
            "native-multi-handler",
            "vault",
            "process_deposit",
            Instruction,
        )
        .auth(
            "signer",
            Some("payer"),
            "programs/vault/src/lib.rs",
            "if !payer.is_signer",
            "native-check",
        ),
        Case::new(
            "native-multi-handler",
            "vault",
            "process_close",
            Instruction,
        )
        .auth(
            "none",
            None,
            "programs/vault/src/lib.rs",
            "pub fn process_close",
            "native-check",
        ),
        Case::new("native-impl-handler", "vault", "process", Instruction).auth(
            "signer",
            Some("payer"),
            "programs/vault/src/lib.rs",
            "if !payer.is_signer",
            "native-check",
        ),
        Case::new("native-truncated", "vault", "process_withdraw", Instruction).auth(
            "signer",
            Some("authority"),
            "programs/vault/src/lib.rs",
            "require!(authority.is_signer",
            "native-check",
        ),
        // --- Solidity -----------------------------------------------------
        Case::new("sol-only-owner", "Vault", "withdraw", Function).auth(
            "owner",
            None,
            "src/Vault.sol",
            "function withdraw() external onlyOwner",
            "solidity-modifier",
        ),
        Case::new("sol-only-owner-defined", "Vault", "withdraw", Function).auth(
            "owner",
            None,
            "src/Vault.sol",
            "_checkOwner();",
            "solidity-modifier",
        ),
        Case::new("sol-only-role", "Vault", "mint", Function).auth(
            "role",
            Some("MINTER_ROLE"),
            "src/Vault.sol",
            "function mint() external onlyRole(MINTER_ROLE)",
            "solidity-modifier",
        ),
        Case::new("sol-custom-modifier-role", "Vault", "pause", Function).auth(
            "role",
            Some("admin"),
            "src/Vault.sol",
            "require(msg.sender == admin",
            "solidity-modifier",
        ),
        Case::new("sol-custom-modifier-owner", "Vault", "close", Function).auth(
            "owner",
            None,
            "src/Vault.sol",
            "require(msg.sender == owner,",
            "solidity-modifier",
        ),
        Case::new("sol-inherited-modifier", "Vault", "poke", Function).auth(
            "role",
            Some("KEEPER"),
            "src/Vault.sol",
            "require(hasRole(KEEPER, msg.sender)",
            "solidity-modifier",
        ),
        Case::new("sol-inherited-most-derived", "Vault", "close", Function).auth(
            "role",
            Some("vaultAdmin"),
            "src/Vault.sol",
            "require(msg.sender == vaultAdmin",
            "solidity-modifier",
        ),
        Case::new("sol-body-require-owner", "Vault", "close", Function).auth(
            "owner",
            None,
            "src/Vault.sol",
            "require(msg.sender == owner()",
            "solidity-body",
        ),
        // A public view function is an entry point; it requires no authority.
        Case::new("sol-body-require-owner", "Vault", "owner", Function).auth(
            "none",
            None,
            "src/Vault.sol",
            "function owner() public view",
            "solidity-body",
        ),
        Case::new("sol-body-has-role", "Vault", "mint", Function).auth(
            "role",
            Some("MINTER"),
            "src/Vault.sol",
            "require(hasRole(MINTER, msg.sender)",
            "solidity-body",
        ),
        Case::new("sol-body-if-revert", "Vault", "close", Function).auth(
            "owner",
            None,
            "src/Vault.sol",
            "if (msg.sender != owner)",
            "solidity-body",
        ),
        Case::new("sol-unresolved-modifier", "Vault", "close", Function).auth(
            "unknown",
            None,
            "src/Vault.sol",
            "function close() external onlyAdmin",
            "solidity-modifier",
        ),
        Case::new("sol-unrecognised-guard", "Vault", "close", Function).auth(
            "unknown",
            None,
            "src/Vault.sol",
            "require(isAllowed(msg.sender)",
            "solidity-body",
        ),
        Case::new("sol-disjunction", "Vault", "close", Function).auth(
            "unknown",
            None,
            "src/Vault.sol",
            "require(msg.sender == owner || msg.sender == admin",
            "solidity-body",
        ),
        Case::new("sol-no-check", "Vault", "open", Function).auth(
            "none",
            None,
            "src/Vault.sol",
            "function open() external",
            "solidity-body",
        ),
        Case::new("sol-non-external", "Vault", "view_only", Function).auth(
            "none",
            None,
            "src/Vault.sol",
            "function view_only() public view",
            "solidity-body",
        ),
        Case::new("sol-non-external", "Vault", "open", Function).auth(
            "none",
            None,
            "src/Vault.sol",
            "function open() external",
            "solidity-body",
        ),
        Case::new("sol-zero-address", "Vault", "open", Function).auth(
            "none",
            None,
            "src/Vault.sol",
            "function open() external",
            "solidity-body",
        ),
        Case::new("sol-multi-contract", "Vault", "withdraw", Function).auth(
            "owner",
            None,
            "src/Vault.sol",
            "function withdraw() external onlyOwner",
            "solidity-modifier",
        ),
        Case::new("sol-multi-contract", "Registry", "register", Function).auth(
            "owner",
            None,
            "src/Vault.sol",
            "function register() external onlyOwner",
            "solidity-modifier",
        ),
        Case::new("sol-inheritance-clean", "Base", "pause", Function).auth(
            "owner",
            None,
            "src/Vault.sol",
            "function pause() external onlyOwner",
            "solidity-modifier",
        ),
        Case::new("sol-inheritance-clean", "Vault", "open", Function).auth(
            "none",
            None,
            "src/Vault.sol",
            "function open() external",
            "solidity-body",
        ),
        // --- Mixed language ----------------------------------------------
        Case::new("mixed-anchor-solidity", "vault", "withdraw", Instruction).auth(
            "signer",
            Some("authority"),
            "src/lib.rs",
            "pub authority: Signer<'info>,",
            "anchor-struct",
        ),
        Case::new("mixed-anchor-solidity", "Vault", "withdraw", Function).auth(
            "owner",
            None,
            "src/Vault.sol",
            "function withdraw() external onlyOwner",
            "solidity-modifier",
        ),
    ]
}

fn resolve(fixture: &str) -> ProgramModel {
    ProgramModel::resolve(&fixture_path(fixture))
        .unwrap_or_else(|error| panic!("{fixture}: {error}"))
}

#[test]
fn every_fixture_resolves_to_exactly_its_expected_authorities() {
    let cases = cases();
    let mut by_fixture: BTreeMap<&str, Vec<&Case>> = BTreeMap::new();
    for case in &cases {
        by_fixture.entry(case.fixture).or_default().push(case);
    }

    let mut problems = Vec::new();
    for (fixture, fixture_cases) in &by_fixture {
        let model = resolve(fixture);
        for case in fixture_cases {
            let Some(entry) = model.entry_point(case.program, case.entry) else {
                problems.push(format!(
                    "{fixture}: entry point {}.{} not found; have {:?}",
                    case.program,
                    case.entry,
                    model
                        .programs
                        .iter()
                        .map(|program| program.name.clone())
                        .collect::<Vec<_>>()
                ));
                continue;
            };
            if entry.kind != case.kind {
                problems.push(format!(
                    "{fixture}: {}.{} kind {:?} != {:?}",
                    case.program, case.entry, entry.kind, case.kind
                ));
            }
            let expected: BTreeSet<(String, Option<String>, String, u32, String)> = case
                .expected
                .iter()
                .map(|expected| {
                    (
                        expected.kind.to_string(),
                        expected.name.map(ToString::to_string),
                        expected.file.to_string(),
                        expected.line,
                        expected.provenance.to_string(),
                    )
                })
                .collect();
            let actual = authority_rows(entry);
            if actual != expected {
                problems.push(format!(
                    "{fixture}: {}.{}\n  expected {expected:?}\n  actual   {actual:?}",
                    case.program, case.entry
                ));
            }
        }

        // Every entry point must be covered by a case: a spurious entry point
        // (an internal function, a helper) fails here.
        let covered: BTreeSet<(&str, &str)> = fixture_cases
            .iter()
            .map(|case| (case.program, case.entry))
            .collect();
        for program in &model.programs {
            for entry in &program.entry_points {
                if !covered.contains(&(program.name.as_str(), entry.name.as_str())) {
                    problems.push(format!(
                        "{fixture}: unresolved entry point {}.{} is not in the matrix",
                        program.name, entry.name
                    ));
                }
            }
        }
    }

    assert!(problems.is_empty(), "{}", problems.join("\n"));
}

#[test]
fn the_matrix_has_at_least_forty_synthetic_programs() {
    let fixtures: BTreeSet<&str> = cases().iter().map(|case| case.fixture).collect();
    assert!(
        fixtures.len() >= 40,
        "the fixture matrix has {} synthetic programs, fewer than 40",
        fixtures.len()
    );
    // Every fixture directory is in the matrix; a new fixture without a case
    // is a gap, not a licence.
    let mut on_disk: BTreeSet<String> = BTreeSet::new();
    for entry in std::fs::read_dir(fixtures_root()).unwrap() {
        let entry = entry.unwrap();
        if entry.file_type().unwrap().is_dir() {
            on_disk.insert(entry.file_name().to_string_lossy().to_string());
        }
    }
    let matrix: BTreeSet<String> = fixtures.iter().map(ToString::to_string).collect();
    assert_eq!(
        on_disk, matrix,
        "the fixture directory and the matrix disagree"
    );
}

#[test]
fn unknown_never_accompanies_a_guessed_authority() {
    for fixture in cases()
        .iter()
        .map(|case| case.fixture)
        .collect::<BTreeSet<_>>()
    {
        let model = resolve(fixture);
        for entry in model.entry_points() {
            let unknowns = entry
                .authorities
                .iter()
                .filter(|authority| matches!(authority, Authority::Unknown { .. }))
                .count();
            if unknowns > 0 {
                assert_eq!(
                    entry.authorities.len(),
                    1,
                    "{fixture}: {}.{} mixes Unknown with a guessed authority: {:?}",
                    entry.program,
                    entry.name,
                    entry.authorities
                );
                assert!(
                    !entry.unknown_reasons.is_empty(),
                    "{fixture}: {}.{} is Unknown with no reason",
                    entry.program,
                    entry.name
                );
                for reason in &entry.unknown_reasons {
                    assert!(!reason.trim().is_empty(), "{fixture}: empty unknown reason");
                }
            }
        }
    }
}

#[test]
fn a_resolution_failure_is_unknown_with_a_reason_not_none() {
    for (fixture, reason_fragment) in [
        ("anchor-missing-struct", "not found"),
        ("anchor-duplicate-struct", "defined more than once"),
        ("anchor-unparsable-accounts", "not found"),
        ("sol-unresolved-modifier", "not defined"),
        ("sol-unrecognised-guard", "unrecognised"),
        ("sol-disjunction", "disjunctive"),
        ("native-disjunctive", "disjunctive"),
        ("native-unnameable", "cannot name"),
    ] {
        let model = resolve(fixture);
        let unknown: Vec<&rg_program::EntryPoint> = model
            .entry_points()
            .filter(|entry| entry.authorities.iter().any(Authority::is_unknown))
            .collect();
        assert_eq!(
            unknown.len(),
            1,
            "{fixture}: expected exactly one Unknown entry point, got {unknown:?}"
        );
        let entry = unknown[0];
        assert_eq!(entry.authorities.len(), 1, "{fixture}");
        assert_eq!(entry.authorities[0].kind(), "unknown", "{fixture}");
        assert!(
            entry
                .unknown_reasons
                .iter()
                .any(|reason| reason.contains(reason_fragment)),
            "{fixture}: reasons {:?} do not mention {reason_fragment:?}",
            entry.unknown_reasons
        );
    }
}

#[test]
fn a_pda_records_its_seeds_bump_and_signer_binding() {
    let model = resolve("anchor-pda");
    let entry = model.entry_point("vault", "initialize").unwrap();
    assert_eq!(entry.pdas.len(), 1);
    let pda = &entry.pdas[0];
    assert_eq!(pda.account, "vault");
    assert_eq!(pda.seeds.len(), 3);
    assert_eq!(pda.bump.as_deref(), Some("bump"));
    assert_eq!(pda.binds_signer, vec!["user".to_string()]);
    assert_eq!(
        pda.span.line,
        marker_line("anchor-pda", "src/lib.rs", "seeds = [b\"vault\"")
    );
}

#[test]
fn a_pda_without_a_signer_seed_does_not_bind_one() {
    let model = resolve("anchor-pda-no-signer");
    let entry = model.entry_point("vault", "init").unwrap();
    assert_eq!(entry.pdas.len(), 1);
    let pda = &entry.pdas[0];
    assert!(pda.binds_signer.is_empty(), "{:?}", pda.binds_signer);
    assert_eq!(pda.bump.as_deref(), Some("bump"));
}

#[test]
fn account_constraints_are_recorded_without_becoming_authority() {
    let model = resolve("anchor-address-attr");
    let entry = model.entry_point("vault", "init").unwrap();
    assert_eq!(entry.authorities[0].kind(), "none");
    assert_eq!(entry.constraints.len(), 1);
    let constraint = &entry.constraints[0];
    assert_eq!(constraint.account, "config");
    assert!(matches!(
        &constraint.kind,
        rg_program::ConstraintKind::AtAddress { expression } if expression == "crate::ID"
    ));

    let model = resolve("native-owner-check");
    let entry = model.entry_point("vault", "process_close").unwrap();
    assert_eq!(entry.authorities[0].kind(), "none");
    assert!(entry.constraints.iter().any(|constraint| matches!(
        &constraint.kind,
        rg_program::ConstraintKind::OwnedBy { expression } if expression == "program_id"
    )));
}

#[test]
fn the_source_wins_over_the_idl_and_says_so() {
    let model = resolve("anchor-idl-conflict");
    let entry = model.entry_point("vault", "withdraw").unwrap();
    assert_eq!(entry.authorities.len(), 1);
    assert_eq!(entry.authorities[0].kind(), "signer");
    assert_eq!(entry.authorities[0].name(), Some("authority"));
    assert!(
        entry
            .notes
            .iter()
            .any(|note| note.contains("conflict: source-over-idl") && note.contains("authority")),
        "{:?}",
        entry.notes
    );
    assert!(
        entry.notes.iter().any(|note| note.contains("extra")),
        "an IDL-only account must be reported: {:?}",
        entry.notes
    );
}

#[test]
fn an_idl_fills_a_missing_accounts_struct() {
    let model = resolve("anchor-idl-fill");
    let entry = model.entry_point("vault", "open").unwrap();
    assert_eq!(entry.authorities.len(), 1);
    assert_eq!(entry.authorities[0].kind(), "signer");
    assert_eq!(entry.authorities[0].name(), Some("admin"));
    assert_eq!(entry.authorities[0].provenance(), Provenance::Idl);
    assert!(
        entry
            .notes
            .iter()
            .any(|note| note.contains("filled from IDL")),
        "{:?}",
        entry.notes
    );
}

#[test]
fn an_anchor_program_id_is_a_lookup_key() {
    let model = resolve("anchor-signer-direct");
    assert!(
        model
            .entry_point("Vau1t1111111111111111111111111111111111111", "withdraw")
            .is_some()
    );
    let program = &model.programs[0];
    assert_eq!(program.language, Language::Anchor);
    assert_eq!(
        program.id.as_deref(),
        Some("Vau1t1111111111111111111111111111111111111")
    );
}
