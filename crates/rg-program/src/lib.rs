#![forbid(unsafe_code)]
//! The program model: what authority each entry point requires.
//!
//! A rule that says "the attacker needed a role" is only as good as its
//! answer to *which* role, *where* the requirement is written, and *how sure*
//! the resolver is. [`Authority::Unknown`] is a first-class answer with a
//! reason, never a silent guess.
//!
//! [`EntryPoint`] and [`Authority`] are deliberately independent of any one
//! language. A Solidity function with `onlyOwner`, an Anchor instruction with
//! a `Signer<'info>` and a native Solana handler that checks `is_signer` all
//! produce the same shape, so the exploit model in `rg-model` can consume them
//! without caring which frontend produced them.
//!
//! # Resolution rules
//!
//! ## Anchor
//!
//! For each `#[program]` module, each `pub fn name(ctx: Context<T>, ...)` is an
//! instruction whose accounts are the `#[derive(Accounts)]` struct `T`.
//!
//! - A field whose account wrapper is `Signer<'info>` (also inside `Box`) is
//!   `Signer(field)`. `Option<Signer<'info>>` is optional and is not a
//!   requirement.
//! - `#[account(signer)]` and `#[account(constraint = x.is_signer)]` are
//!   `Signer(x)`.
//! - `#[account(has_one = x)]` is `Owner(x)`: the signer of `x` must be the
//!   key the account stores. Several `has_one` entries form a chain and each
//!   contributes an `Owner`.
//! - `#[account(owner = e)]` is `Owner(e)`: the account's owner is `e`.
//! - `#[account(constraint = a.key() == b.key())]` binds two accounts. When
//!   exactly one side names a signer field, the caller must present the key on
//!   the other side, which is `Role(other)`.
//! - `#[account(seeds = [...], bump)]` is a PDA: the seeds, the bump and the
//!   signer fields the seeds are derived from are recorded on the entry point.
//! - `#[account(address = e)]` is recorded as a constraint, not an authority:
//!   a fixed address is an account requirement, and only when the expression
//!   names a signer field does the constraint bind one.
//! - An accounts struct that is missing, defined twice, or carries an
//!   `#[account(...)]` attribute the parser cannot read is `Unknown`, never a
//!   partial answer.
//!
//! `target/idl/*.json` is read when present. The Rust source wins: when the
//! IDL's `isSigner` disagrees with a field, the entry point keeps the source
//! answer and records `conflict: source-over-idl: ...` in its notes. When the
//! accounts struct is not in the source at all and the IDL lists the
//! instruction, the IDL's `isSigner` flags fill the authorities, with
//! provenance [`Provenance::Idl`] and a note that says so.
//!
//! ## Native Solana
//!
//! A function that takes an `AccountInfo` slice or `Vec<AccountInfo>` is an
//! instruction handler, and each handler is an entry point on its own: a
//! dispatcher and the per-instruction functions it calls are reported
//! separately rather than merged into one answer. The exception is an
//! accessor: a function that takes accounts, is called by another handler in
//! the same program and does not return a `Result` (for example
//! `get_account(accounts, 0)`) is not an instruction. Checks are read from the
//! handler's own body:
//!
//! - `require!(x.is_signer)`, `assert!(x.is_signer)` and
//!   `if !x.is_signer { return Err(...) }` are `Signer(x)`.
//! - `x.is_signer` as a *positive* `if` condition is a conditional privileged
//!   path, not a requirement; it contributes nothing.
//! - `a.key == b.key` where one side is a signer-checked account is
//!   `Role(other)`; other key and owner comparisons are recorded as
//!   constraints.
//! - A disjunctive signer check (`a.is_signer || b.is_signer`, or the negation
//!   of a conjunction) requires at least one of several authorities; the model
//!   cannot name a set, so the answer is `Unknown`.
//! - An `is_signer` check on an expression the model cannot name (a call
//!   result, for example) is `Unknown`.
//!
//! ## Solidity
//!
//! With `solar` on the target tree, each external or public function with a
//! body in a contract (not an interface or library) in a source file outside
//! Foundry `lib/` is an entry point. For each one:
//!
//! - `onlyOwner` is `Owner`; `onlyRole(X)` is `Role(X)`. A canonical modifier
//!   whose definition is in the tree is resolved from its body instead, and a
//!   body that checks nothing contributes nothing.
//! - Any other modifier must be found in the contract's inheritance chain
//!   (the same most-derived-first linearisation `project.rs` uses); its body
//!   is resolved to its `require`/`if ... revert` statements. A modifier that
//!   cannot be found is `Unknown`.
//! - In-body (and directly called helper) `require`/`assert`/`if ... revert`
//!   guards: `msg.sender == owner()` is `Owner`; `hasRole(R, msg.sender)` is
//!   `Role(R)`; `msg.sender == e` is `Role(e)`. `address(0)` and
//!   `address(this)` comparisons are input validation and self-call gates, not
//!   caller authority. A demanded disjunction, or a guard on `msg.sender` the
//!   model does not recognise, is `Unknown`.
//! - Modifier arguments are substituted for the modifier's parameters, so
//!   `onlyRole(ADMIN_ROLE)` resolves to `Role(ADMIN_ROLE)` whether the body is
//!   in the tree or not.
//!
//! # Confinement and determinism
//!
//! [`ProgramModel::resolve`] takes a target directory; every read goes through
//! [`TargetRoot`], which refuses `..`, absolute paths and symlinks that leave
//! the root. The model is `serde`-serialisable and its serialisation is
//! byte-identical across runs: the directory walk is sorted, every collection
//! is sorted or kept in source order, and no hash map is serialised.

mod anchor;
mod native;
mod solidity;
mod synutil;
pub mod target;

use serde::{Deserialize, Serialize};
use std::fmt;
use std::path::{Path, PathBuf};

pub use target::{
    MAX_SOURCE_FILE_BYTES, MAX_SOURCE_FILES, MAX_SOURCE_TOTAL_BYTES, MAX_WALK_DEPTH, TargetRoot,
};

/// Where a requirement was found.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Provenance {
    /// An Anchor IDL (`target/idl/*.json`).
    Idl,
    /// An Anchor `#[derive(Accounts)]` struct.
    AnchorStruct,
    /// A native Solana `is_signer`, owner or key check.
    NativeCheck,
    /// A Solidity modifier, applied or inherited.
    SolidityModifier,
    /// A Solidity function body (`require`, `if ... revert`).
    SolidityBody,
}

/// A position in a source file under the target, with the text at it.
///
/// `file` is relative to the target root and uses forward slashes; the model
/// never carries an absolute host path. Lines and columns are 1-based.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Span {
    pub file: String,
    pub line: u32,
    pub column: u32,
    /// The source text at the span, whitespace-collapsed and capped at 160
    /// characters. Empty when the span is not a source position (an IDL).
    pub text: String,
}

/// The authority a caller must hold to reach an entry point.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Authority {
    /// A signature by the named account: an Anchor `Signer<'info>`, a native
    /// `is_signer` check, or a Solidity caller the resolver can name.
    Signer {
        account: String,
        spans: Vec<Span>,
        provenance: Provenance,
    },
    /// Membership in the named role: Solidity `onlyRole(X)` or `hasRole`, or
    /// an Anchor key constraint that pins a signer to a configured key.
    Role {
        name: String,
        spans: Vec<Span>,
        provenance: Provenance,
    },
    /// The `onlyOwner`-style owner: Solidity `owner()` / `onlyOwner`, an
    /// Anchor `has_one` chain, or an Anchor `owner =` constraint. The concrete
    /// account or expression is in the entry point's constraints and in the
    /// span's text.
    Owner {
        spans: Vec<Span>,
        provenance: Provenance,
    },
    /// No caller-held authority is required. This is an answer, not a default:
    /// a resolver that could not decide returns [`Authority::Unknown`].
    None {
        spans: Vec<Span>,
        provenance: Provenance,
    },
    /// The resolver could not decide, and says why. Never accompanies a
    /// guessed authority: when any part of the answer is unresolved, the
    /// entry point's authority list is exactly this one variant.
    Unknown {
        reason: String,
        spans: Vec<Span>,
        provenance: Provenance,
    },
}

impl Authority {
    /// The JSON `kind` of this authority: `signer`, `role`, `owner`, `none` or
    /// `unknown`.
    pub fn kind(&self) -> &'static str {
        match self {
            Authority::Signer { .. } => "signer",
            Authority::Role { .. } => "role",
            Authority::Owner { .. } => "owner",
            Authority::None { .. } => "none",
            Authority::Unknown { .. } => "unknown",
        }
    }

    /// The account or role name, when the variant carries one.
    pub fn name(&self) -> Option<&str> {
        match self {
            Authority::Signer { account, .. } => Some(account),
            Authority::Role { name, .. } => Some(name),
            Authority::Owner { .. } | Authority::None { .. } | Authority::Unknown { .. } => None,
        }
    }

    /// Every span the requirement was found at, in discovery order.
    pub fn spans(&self) -> &[Span] {
        match self {
            Authority::Signer { spans, .. }
            | Authority::Role { spans, .. }
            | Authority::Owner { spans, .. }
            | Authority::None { spans, .. }
            | Authority::Unknown { spans, .. } => spans,
        }
    }

    /// Where the requirement came from.
    pub fn provenance(&self) -> Provenance {
        match self {
            Authority::Signer { provenance, .. }
            | Authority::Role { provenance, .. }
            | Authority::Owner { provenance, .. }
            | Authority::None { provenance, .. }
            | Authority::Unknown { provenance, .. } => *provenance,
        }
    }

    pub fn is_unknown(&self) -> bool {
        matches!(self, Authority::Unknown { .. })
    }

    /// Adds the spans of a duplicate requirement to this one, keeping the
    /// first provenance and dropping spans already present.
    fn absorb(&mut self, other: &Authority) {
        if !same_requirement(self, other) {
            return;
        }
        let extra: Vec<Span> = other
            .spans()
            .iter()
            .filter(|span| !self.spans().contains(span))
            .cloned()
            .collect();
        match self {
            Authority::Signer { spans, .. }
            | Authority::Role { spans, .. }
            | Authority::Owner { spans, .. }
            | Authority::None { spans, .. }
            | Authority::Unknown { spans, .. } => spans.extend(extra),
        }
    }
}

fn same_requirement(a: &Authority, b: &Authority) -> bool {
    match (a, b) {
        (Authority::Signer { account: x, .. }, Authority::Signer { account: y, .. }) => x == y,
        (Authority::Role { name: x, .. }, Authority::Role { name: y, .. }) => x == y,
        (Authority::Owner { .. }, Authority::Owner { .. }) => true,
        (Authority::None { .. }, Authority::None { .. }) => true,
        (Authority::Unknown { reason: x, .. }, Authority::Unknown { reason: y, .. }) => x == y,
        _ => false,
    }
}

/// Merges duplicate requirements, keeping discovery order and combining spans.
fn dedupe(mut authorities: Vec<Authority>) -> Vec<Authority> {
    let mut out: Vec<Authority> = Vec::new();
    for authority in authorities.drain(..) {
        match out
            .iter_mut()
            .find(|seen| same_requirement(seen, &authority))
        {
            Some(seen) => seen.absorb(&authority),
            None => out.push(authority),
        }
    }
    out
}

/// One PDA constraint on an Anchor accounts struct.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PdaConstraint {
    /// The accounts-struct field the PDA is derived for.
    pub account: String,
    /// The seed expressions, as written.
    pub seeds: Vec<String>,
    /// `bump`, `bump = expr`, or `None` when the seeds carry no bump.
    pub bump: Option<String>,
    /// Signer fields the seeds are derived from (`seeds = [user.key()...]`).
    pub binds_signer: Vec<String>,
    pub span: Span,
}

/// An account constraint read from the source.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ConstraintKind {
    /// Anchor `has_one = x` (or several, a chain).
    HasOne { account: String },
    /// `a.key() == b.key()` (Anchor constraint or native comparison).
    KeyEquals { left: String, right: String },
    /// `owner = e` (Anchor) or `account.owner == e` (native).
    OwnedBy { expression: String },
    /// Anchor `address = e`.
    AtAddress { expression: String },
}

/// One account constraint and where it is written.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AccountConstraint {
    /// The accounts-struct field (or account expression) constrained.
    pub account: String,
    pub kind: ConstraintKind,
    pub span: Span,
}

/// Whether an entry point is a Solana instruction or a Solidity function.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EntryPointKind {
    Instruction,
    Function,
}

/// The language the entry points came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum Language {
    #[serde(rename = "anchor")]
    Anchor,
    #[serde(rename = "native")]
    NativeSolana,
    #[serde(rename = "solidity")]
    Solidity,
}

impl Language {
    fn rank(self) -> u8 {
        match self {
            Language::Anchor => 0,
            Language::NativeSolana => 1,
            Language::Solidity => 2,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Language::Anchor => "anchor",
            Language::NativeSolana => "native",
            Language::Solidity => "solidity",
        }
    }
}

/// One externally reachable entry point and its required authorities.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EntryPoint {
    /// The program, module or contract the entry point belongs to.
    pub program: String,
    /// The instruction or function name.
    pub name: String,
    pub kind: EntryPointKind,
    /// Every authority the entry point requires. An entry point that checked
    /// and found none carries exactly [`Authority::None`]; an unresolved one
    /// carries exactly [`Authority::Unknown`].
    pub authorities: Vec<Authority>,
    /// One entry per reason the answer is unknown. Empty when it is not.
    pub unknown_reasons: Vec<String>,
    /// Where the declaration starts.
    pub source_span: Span,
    /// Anchor PDA accounts, with seeds and bump.
    pub pdas: Vec<PdaConstraint>,
    /// Account constraints the model read but that are not caller authority.
    pub constraints: Vec<AccountConstraint>,
    /// Non-authority observations: IDL conflicts, optional signers, skipped
    /// checks. This is where "the source wins over the IDL" is recorded.
    pub notes: Vec<String>,
}

impl EntryPoint {
    fn new(program: &str, name: &str, kind: EntryPointKind, source_span: Span) -> Self {
        Self {
            program: program.to_string(),
            name: name.to_string(),
            kind,
            authorities: Vec::new(),
            unknown_reasons: Vec::new(),
            source_span,
            pdas: Vec::new(),
            constraints: Vec::new(),
            notes: Vec::new(),
        }
    }

    /// Sets the authorities, folding duplicates and replacing "no authorities
    /// found" with the explicit [`Authority::None`] answer.
    fn with_authorities(mut self, authorities: Vec<Authority>, none: Authority) -> Self {
        let deduped = dedupe(authorities);
        self.authorities = if deduped.is_empty() {
            vec![none]
        } else {
            deduped
        };
        self
    }

    /// Records an unresolved answer. Any known authorities are dropped: a
    /// partial answer pretends the unknown part does not exist.
    fn unknown(mut self, reason: String, span: Span, provenance: Provenance) -> Self {
        self.unknown_reasons.push(reason.clone());
        self.authorities = vec![Authority::Unknown {
            reason,
            spans: vec![span],
            provenance,
        }];
        self
    }
}

/// One program or contract and its entry points.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Program {
    /// The Anchor module name, the native program directory name, or the
    /// Solidity contract name.
    pub name: String,
    /// The Anchor `declare_id!` program id, when the source names one.
    pub id: Option<String>,
    pub language: Language,
    pub entry_points: Vec<EntryPoint>,
}

/// Every program under a target and their entry points.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProgramModel {
    /// The target directory's own name. The absolute path is never recorded.
    pub target: String,
    /// Sorted by language, then name; entry points sorted by source position.
    pub programs: Vec<Program>,
    /// Model-wide notes: files skipped, parse failures, duplicate names.
    pub notes: Vec<String>,
}

/// How [`ProgramModel::resolve_with`] should read the target.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ResolveOptions {
    /// A git revision to read instead of the working tree. Not supported: the
    /// model reads a checked-out tree, so a caller that needs a pinned commit
    /// checks it out first and passes that directory.
    pub commit: Option<String>,
}

impl ProgramModel {
    /// Resolves every entry point under `target`.
    pub fn resolve(target: &Path) -> Result<Self, ProgramError> {
        Self::resolve_with(target, &ResolveOptions::default())
    }

    /// Resolves every entry point under `target`, with options.
    pub fn resolve_with(target: &Path, options: &ResolveOptions) -> Result<Self, ProgramError> {
        if let Some(commit) = &options.commit {
            return Err(ProgramError::Unsupported(format!(
                "pinned commit `{commit}` is not supported by the program model; check the commit \
                 out and pass that directory"
            )));
        }
        let root = TargetRoot::new(target)?;
        let inventory = root.inventory()?;
        let mut notes = inventory.notes;

        let mut rust_files: Vec<(String, String)> = Vec::new();
        for relative in &inventory.rust {
            match root.read(relative) {
                Ok(source) => rust_files.push((relative.clone(), source)),
                Err(error) => notes.push(format!("{relative}: {}", error.reason())),
            }
        }
        let mut solidity_files: Vec<(String, String)> = Vec::new();
        for relative in inventory
            .solidity
            .iter()
            .chain(inventory.solidity_deps.iter())
        {
            match root.read(relative) {
                Ok(source) => solidity_files.push((relative.clone(), source)),
                Err(error) => notes.push(format!("{relative}: {}", error.reason())),
            }
        }
        let mut idl_texts: Vec<(String, String)> = Vec::new();
        for relative in &inventory.idl {
            match root.read(relative) {
                Ok(text) => idl_texts.push((relative.clone(), text)),
                Err(error) => notes.push(format!("{relative}: {}", error.reason())),
            }
        }

        let (mut programs, anchor_notes) = anchor::collect(&rust_files, &idl_texts);
        notes.extend(anchor_notes);
        let (native_programs, native_notes) = native::collect(&rust_files);
        notes.extend(native_notes);
        let (solidity_programs, solidity_notes) =
            solidity::collect(&solidity_files, &inventory.solidity);
        notes.extend(solidity_notes);
        programs.extend(native_programs);
        programs.extend(solidity_programs);
        programs.retain(|program| !program.entry_points.is_empty());

        if programs.is_empty() {
            return Err(ProgramError::Unsupported(format!(
                "no Anchor program, native Solana handler or Solidity contract was found under \
                 the target `{}`",
                root.name()
            )));
        }

        for program in &mut programs {
            program.entry_points.sort_by(|a, b| {
                (
                    &a.source_span.file,
                    a.source_span.line,
                    a.source_span.column,
                    &a.name,
                )
                    .cmp(&(
                        &b.source_span.file,
                        b.source_span.line,
                        b.source_span.column,
                        &b.name,
                    ))
            });
        }
        programs.sort_by(|a, b| {
            (a.language.rank(), &a.name, &a.id).cmp(&(b.language.rank(), &b.name, &b.id))
        });
        notes.sort();
        notes.dedup();
        Ok(Self {
            target: root.name().to_string(),
            programs,
            notes,
        })
    }

    /// Looks up one entry point by program id or name, then by entry point
    /// name. Returns the first match in the model's deterministic order.
    pub fn entry_point(&self, program: &str, name: &str) -> Option<&EntryPoint> {
        self.programs
            .iter()
            .find(|candidate| candidate.name == program || candidate.id.as_deref() == Some(program))
            .and_then(|candidate| {
                candidate
                    .entry_points
                    .iter()
                    .find(|entry| entry.name == name)
            })
    }

    /// Every entry point in every program.
    pub fn entry_points(&self) -> impl Iterator<Item = &EntryPoint> {
        self.programs
            .iter()
            .flat_map(|program| &program.entry_points)
    }

    /// The deterministic JSON form used by `--dump-program`. Serialisation of
    /// these types cannot fail; the panic message says so if that changes.
    pub fn to_json_pretty(&self) -> String {
        serde_json::to_string_pretty(self).expect("ProgramModel is serialisable")
    }
}

/// Why a program model could not answer.
#[derive(Debug)]
pub enum ProgramError {
    /// The target tree could not be read.
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
    /// The target uses a language, framework or layout the resolver does not
    /// support. The string says which.
    Unsupported(String),
    /// A path left the target root: an absolute path, a `..` component, or a
    /// symlink whose target is outside.
    OutsideTarget {
        path: String,
        root: PathBuf,
    },
    /// A source file is over the per-file size cap.
    InputTooLarge {
        path: String,
        bytes: u64,
        limit: u64,
    },
    /// The source tree is over the total size or file-count cap.
    TotalTooLarge {
        bytes: u64,
        limit: u64,
    },
    TooManyFiles {
        limit: usize,
    },
    /// Directory nesting is deeper than the walk cap.
    TooDeep {
        path: PathBuf,
        limit: usize,
    },
    /// A file that should be source is not a regular file.
    NotRegular {
        path: String,
    },
    /// A source file is not valid UTF-8.
    NotUtf8 {
        path: String,
    },
}

impl ProgramError {
    /// The error without a filesystem path, for a note that is serialised
    /// into the model. The caller prefixes the relative path.
    pub fn reason(&self) -> String {
        match self {
            ProgramError::Io { source, .. } => source.to_string(),
            ProgramError::Unsupported(message) => message.clone(),
            ProgramError::OutsideTarget { .. } => "outside the target root".to_string(),
            ProgramError::InputTooLarge { bytes, limit, .. } => {
                format!("{bytes} bytes is over the {limit}-byte per-file limit")
            }
            ProgramError::TotalTooLarge { bytes, limit } => {
                format!("the sources total {bytes} bytes, over the {limit}-byte total limit")
            }
            ProgramError::TooManyFiles { limit } => {
                format!("more than {limit} source files; that is the per-run file limit")
            }
            ProgramError::TooDeep { limit, .. } => {
                format!("directory nesting is deeper than the {limit}-level depth limit")
            }
            ProgramError::NotRegular { .. } => "not a regular file".to_string(),
            ProgramError::NotUtf8 { .. } => "not valid UTF-8".to_string(),
        }
    }
}

impl fmt::Display for ProgramError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ProgramError::Io { path, source } => write!(f, "{}: {source}", path.display()),
            ProgramError::Unsupported(message) => write!(f, "{message}"),
            ProgramError::OutsideTarget { path, .. } => {
                write!(f, "{path}: outside the target root")
            }
            ProgramError::InputTooLarge { path, bytes, limit } => write!(
                f,
                "{path}: {bytes} bytes is over the {limit}-byte per-file limit; it was not read"
            ),
            ProgramError::TotalTooLarge { bytes, limit } => write!(
                f,
                "the sources total {bytes} bytes, over the {limit}-byte total limit; the model \
                 was not built"
            ),
            ProgramError::TooManyFiles { limit } => {
                write!(
                    f,
                    "more than {limit} source files; that is the per-run file limit"
                )
            }
            ProgramError::TooDeep { path, limit } => write!(
                f,
                "{}: directory nesting is deeper than the {limit}-level depth limit",
                path.file_name()
                    .map(|name| name.to_string_lossy())
                    .unwrap_or_default()
            ),
            ProgramError::NotRegular { path } => {
                write!(f, "{path}: not a regular file; it was not read")
            }
            ProgramError::NotUtf8 { path } => {
                write!(f, "{path}: not valid UTF-8; it was not parsed")
            }
        }
    }
}

impl std::error::Error for ProgramError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unknown_is_an_answer_with_a_reason() {
        let a = Authority::Unknown {
            reason: "inherited modifier not resolved".to_string(),
            spans: vec![Span {
                file: "src/a.sol".to_string(),
                line: 1,
                column: 1,
                text: "function f()".to_string(),
            }],
            provenance: Provenance::SolidityModifier,
        };
        assert!(a.is_unknown());
        assert_eq!(a.kind(), "unknown");
        assert_eq!(a.name(), None);
        assert_eq!(a.provenance(), Provenance::SolidityModifier);
        assert!(
            !Authority::None {
                spans: Vec::new(),
                provenance: Provenance::SolidityBody
            }
            .is_unknown()
        );
    }

    #[test]
    fn duplicates_merge_spans_in_discovery_order() {
        let first = Span {
            file: "src/lib.rs".to_string(),
            line: 3,
            column: 5,
            text: "Signer<'info>".to_string(),
        };
        let second = Span {
            file: "src/lib.rs".to_string(),
            line: 9,
            column: 5,
            text: "authority.is_signer".to_string(),
        };
        let merged = dedupe(vec![
            Authority::Signer {
                account: "authority".to_string(),
                spans: vec![first.clone()],
                provenance: Provenance::AnchorStruct,
            },
            Authority::Signer {
                account: "authority".to_string(),
                spans: vec![second.clone()],
                provenance: Provenance::NativeCheck,
            },
            Authority::Role {
                name: "admin".to_string(),
                spans: vec![first],
                provenance: Provenance::SolidityBody,
            },
        ]);
        assert_eq!(merged.len(), 2);
        assert_eq!(merged[0].spans(), &[first_span(), second]);
        assert_eq!(merged[0].provenance(), Provenance::AnchorStruct);
    }

    fn first_span() -> Span {
        Span {
            file: "src/lib.rs".to_string(),
            line: 3,
            column: 5,
            text: "Signer<'info>".to_string(),
        }
    }

    #[test]
    fn an_entry_point_carries_every_required_authority() {
        let span = first_span();
        let entry = EntryPoint::new("vault", "withdraw", EntryPointKind::Instruction, span)
            .with_authorities(
                vec![
                    Authority::Signer {
                        account: "authority".to_string(),
                        spans: vec![first_span()],
                        provenance: Provenance::AnchorStruct,
                    },
                    Authority::Role {
                        name: "keeper".to_string(),
                        spans: vec![first_span()],
                        provenance: Provenance::SolidityBody,
                    },
                ],
                Authority::None {
                    spans: vec![first_span()],
                    provenance: Provenance::AnchorStruct,
                },
            );
        assert_eq!(entry.authorities.len(), 2);
        assert!(!entry.authorities.iter().any(Authority::is_unknown));
    }

    #[test]
    fn an_empty_answer_is_the_explicit_none() {
        let span = first_span();
        let entry = EntryPoint::new("vault", "open", EntryPointKind::Function, span.clone())
            .with_authorities(
                Vec::new(),
                Authority::None {
                    spans: vec![span.clone()],
                    provenance: Provenance::SolidityBody,
                },
            );
        assert_eq!(entry.authorities.len(), 1);
        assert_eq!(entry.authorities[0].kind(), "none");
        assert_eq!(entry.authorities[0].spans()[0].line, 3);
    }

    #[test]
    fn a_failed_resolution_drops_known_authorities() {
        let span = first_span();
        let entry = EntryPoint::new("vault", "close", EntryPointKind::Function, span.clone())
            .with_authorities(
                vec![Authority::Signer {
                    account: "authority".to_string(),
                    spans: vec![span.clone()],
                    provenance: Provenance::SolidityModifier,
                }],
                Authority::None {
                    spans: vec![span.clone()],
                    provenance: Provenance::SolidityBody,
                },
            )
            .unknown(
                "modifier `onlyAdmin` not found".to_string(),
                span,
                Provenance::SolidityModifier,
            );
        assert_eq!(entry.authorities.len(), 1);
        assert!(entry.authorities[0].is_unknown());
        assert_eq!(entry.unknown_reasons.len(), 1);
    }

    #[test]
    fn lookup_accepts_a_program_name_or_id() {
        let span = first_span();
        let entry = EntryPoint::new("vault", "withdraw", EntryPointKind::Instruction, span)
            .with_authorities(
                Vec::new(),
                Authority::None {
                    spans: Vec::new(),
                    provenance: Provenance::AnchorStruct,
                },
            );
        let model = ProgramModel {
            target: "fixture".to_string(),
            programs: vec![Program {
                name: "vault".to_string(),
                id: Some("Vau1t1111111111111111111111111111111111111".to_string()),
                language: Language::Anchor,
                entry_points: vec![entry],
            }],
            notes: Vec::new(),
        };
        assert!(model.entry_point("vault", "withdraw").is_some());
        assert!(
            model
                .entry_point("Vau1t1111111111111111111111111111111111111", "withdraw")
                .is_some()
        );
        assert!(model.entry_point("vault", "close").is_none());
        assert!(model.entry_point("other", "withdraw").is_none());
    }

    #[test]
    fn a_pinned_commit_is_refused_rather_than_pretended() {
        let error = ProgramModel::resolve_with(
            Path::new("."),
            &ResolveOptions {
                commit: Some("deadbeef".to_string()),
            },
        )
        .unwrap_err();
        assert!(error.to_string().contains("not supported"));
    }
}
