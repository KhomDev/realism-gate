//! What a call means in a Solana program test.
//!
//! Three harnesses are recognised: `solana-program-test` (`ProgramTest`,
//! `BanksClient`, `ProgramTestContext`), LiteSVM, and Mollusk. They are
//! matched by method and path names, not by type, because the tool never
//! resolves crates: `svm.set_account(..)` is a direct account write whatever
//! `svm` is. A wrapper the test defines itself is inlined, so its calls to the
//! harness are seen where they happen.

use super::trace::Call;
use crate::ir::Expr;

/// Harness methods that are never resolved to a function of the test's own
/// code, so `svm.set_account(..)` stays an account write even when a fixture
/// type defines a method with the same name.
pub const METHODS: &[&str] = &[
    "add_account",
    "add_account_with_base64_data",
    "add_account_with_file_data",
    "add_program",
    "add_program_from_file",
    "add_upgradeable_program_to_genesis",
    "airdrop",
    "get_account",
    "get_balance",
    "latest_blockhash",
    "get_latest_blockhash",
    "process_instruction",
    "process_and_validate_instruction",
    "process_instruction_chain",
    "process_and_validate_instruction_chain",
    "process_transaction",
    "process_transaction_with_metadata",
    "request_airdrop",
    "send_transaction",
    "set_account",
    "simulate_transaction",
    "start",
    "start_with_context",
    "warp_to_slot",
];

/// Mollusk calls that run one or more instructions; the `validate` variants
/// also check the result against a list of `Check`s.
const MOLLUSK_RUNS: &[&str] = &[
    "process_instruction",
    "process_and_validate_instruction",
    "process_instruction_chain",
    "process_and_validate_instruction_chain",
    "process_transaction_instructions",
    "process_and_validate_transaction_instructions",
];

/// What a harness call does to the world the test runs in.
#[derive(Debug, Clone)]
pub enum Event<'p> {
    /// A transaction is built and signed, or Mollusk runs instructions. The
    /// signers are the keypairs whose signatures it carries.
    Transaction {
        signers: Option<&'p Expr>,
        instructions: Option<&'p Expr>,
    },
    /// An account's lamports, data or owner are set directly:
    /// `set_account`, `add_account`.
    WriteAccount {
        target: &'p Expr,
        account: Option<&'p Expr>,
    },
    /// Account data recorded elsewhere is loaded: real state, copied in.
    LoadAccount { target: &'p Expr },
    /// Lamports sent from the harness's faucet. This is a real system
    /// transfer, the same one an attacker could make with their own lamports.
    Airdrop {
        target: &'p Expr,
        amount: Option<&'p Expr>,
    },
    /// A program is loaded at an address.
    LoadProgram { id: &'p Expr, code: Vec<&'p Expr> },
    /// A client for a live cluster is created: what it reads is the chain as
    /// it is when the test runs.
    LiveRpc,
    /// A transaction is sent. When the trace does not show it being built
    /// (it comes from code the tool does not read), the send stands for it.
    Send,
    /// A test harness is constructed.
    Harness,
}

fn path_ends(path: &str, suffix: &[&str]) -> bool {
    let segs: Vec<&str> = path.rsplit("::").collect();
    suffix.len() <= segs.len() && suffix.iter().rev().zip(&segs).all(|(a, b)| a == b)
}

/// The event a call causes, if it is a harness call.
pub fn event<'p>(c: &Call<'p>) -> Option<Event<'p>> {
    let args = c.args;
    let arg = |i: usize| args.get(i);
    let tx = |signers: Option<&'p Expr>, instructions: Option<&'p Expr>| Event::Transaction {
        signers,
        instructions,
    };
    if !c.path.is_empty() {
        let p = c.path.as_str();
        if path_ends(p, &["Transaction", "new_signed_with_payer"]) {
            return Some(tx(arg(2), arg(0)));
        }
        if path_ends(p, &["Transaction", "new"])
            || path_ends(p, &["Transaction", "new_with_compiled_instructions"])
        {
            return Some(tx(arg(0), arg(1)));
        }
        if path_ends(p, &["VersionedTransaction", "try_new"]) {
            return Some(tx(arg(1), arg(0)));
        }
        let segs: Vec<&str> = p.rsplit("::").collect();
        if segs.len() >= 2 && segs[1].ends_with("RpcClient") && segs[0].starts_with("new") {
            return Some(Event::LiveRpc);
        }
        if segs.len() >= 2
            && matches!(segs[1], "ProgramTest" | "LiteSVM" | "Mollusk")
            && matches!(segs[0], "new" | "default")
        {
            if segs[1] == "ProgramTest" && segs[0] == "new" && args.len() >= 2 {
                return Some(Event::LoadProgram {
                    id: &args[1],
                    code: vec![&args[0]],
                });
            }
            if segs[1] == "Mollusk" && segs[0] == "new" && args.len() >= 2 {
                return Some(Event::LoadProgram {
                    id: &args[0],
                    code: vec![&args[1]],
                });
            }
            return Some(Event::Harness);
        }
        return None;
    }
    c.receiver?;
    match (c.name.as_str(), args.len()) {
        ("sign" | "partial_sign" | "try_sign" | "try_partial_sign", 2) => Some(tx(arg(0), None)),
        ("set_account" | "add_account", 2) => Some(Event::WriteAccount {
            target: &args[0],
            account: arg(1),
        }),
        ("add_account_with_file_data" | "add_account_with_base64_data", n) if n >= 1 => {
            Some(Event::LoadAccount { target: &args[0] })
        }
        ("airdrop" | "request_airdrop", n) if n >= 1 => Some(Event::Airdrop {
            target: &args[0],
            amount: arg(1),
        }),
        // ProgramTest::add_program(name, id, processor)
        ("add_program", 3) => Some(Event::LoadProgram {
            id: &args[1],
            code: vec![&args[0], &args[2]],
        }),
        // LiteSVM::add_program(id, bytes), Mollusk::add_program(id, name, ..)
        ("add_program" | "add_program_from_file" | "add_program_with_loader", n) if n >= 2 => {
            Some(Event::LoadProgram {
                id: &args[0],
                code: args[1..].iter().collect(),
            })
        }
        ("add_upgradeable_program_to_genesis", 2) => Some(Event::LoadProgram {
            id: &args[1],
            code: vec![&args[0]],
        }),
        (m, n) if MOLLUSK_RUNS.contains(&m) && n >= 1 => Some(tx(None, arg(0))),
        (
            "send_transaction"
            | "process_transaction"
            | "process_transaction_with_metadata"
            | "process_transaction_with_preflight"
            | "send_and_confirm_transaction",
            1,
        ) => Some(Event::Send),
        ("start" | "start_with_context", 0) => Some(Event::Harness),
        _ => None,
    }
}

/// Mollusk calls that check their result against `Check`s count as
/// assertions.
pub fn is_validating_run(name: &str) -> bool {
    MOLLUSK_RUNS.contains(&name) && name.contains("validate")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn path_suffixes_match_whole_segments() {
        assert!(path_ends(
            "solana_sdk::transaction::Transaction::new_signed_with_payer",
            &["Transaction", "new_signed_with_payer"]
        ));
        assert!(!path_ends(
            "VersionedTransaction::new",
            &["Transaction", "new"]
        ));
        assert!(!path_ends("new", &["Transaction", "new"]));
    }
}
