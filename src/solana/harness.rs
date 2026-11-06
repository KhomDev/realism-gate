//! What a call means in a Solana program test.
//!
//! The harnesses recognised are `solana-program-test` (`ProgramTest`,
//! `BanksClient`, `ProgramTestContext`), LiteSVM, Mollusk, anchor-client and
//! anchor-litesvm in Rust, and Anchor's TypeScript client
//! (`@coral-xyz/anchor`), web3.js, bankrun and LiteSVM in TypeScript. They are
//! matched by method and path names, not by type, because the tool never
//! resolves packages: `svm.set_account(..)` is a direct account write whatever
//! `svm` is. Both languages reach this module in the same spelling (see
//! [`super::parse_ts`]), so `svm.setAccount(..)` is matched as `set_account`.
//! A wrapper the test defines itself is inlined, so its calls to the harness
//! are seen where they happen.

use super::trace::Call;
use crate::identity::words;
use crate::ir::{Expr, ExprKind};

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
    "execute_instruction",
    "execute_instructions",
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
    "rpc",
    "send_and_confirm",
    "send_transaction",
    "set_account",
    "simulate_transaction",
    "start",
    "start_with_context",
    "try_process_transaction",
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

/// The fee payer Anchor's TypeScript provider signs every transaction with:
/// `provider.wallet`. The tool names it `wallet`, as `provider.wallet` is.
pub const PROVIDER_WALLET: &str = "wallet";

/// The payer an anchor-client `Client` signs every request with.
pub const CLIENT_PAYER: &str = "payer";

/// What a harness call does to the world the test runs in.
#[derive(Debug, Clone)]
pub enum Event<'p> {
    /// A transaction is built and signed, sent through an Anchor client, or
    /// Mollusk runs instructions.
    Transaction {
        /// The keypairs whose signatures it carries: signer lists
        /// (`&[&payer, &attacker]`) or single signers, as written.
        signers: Vec<&'p Expr>,
        /// A signer the call adds without naming it: the Anchor provider's
        /// wallet or the anchor-client payer.
        implicit: Option<&'static str>,
        /// What the transaction runs: instructions, instruction lists, or
        /// the Anchor builder call that names the instruction
        /// (`program.methods.withdraw(amount)`).
        instructions: Vec<&'p Expr>,
        /// The accounts an Anchor builder passes:
        /// `.accounts({ vault, authority: admin.publicKey })`.
        accounts: Option<&'p Expr>,
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

/// The method calls of a chain, outermost first, ending where the chain
/// stops being method calls: `program.methods.x(a).signers(s)` gives
/// `signers(s)` then `x(a)`.
fn chain(e: &Expr) -> Vec<(&str, &[Expr], &Expr)> {
    let mut out = Vec::new();
    let mut cur = e;
    while let ExprKind::Call { callee, args, .. } = &cur.kind
        && let ExprKind::Member(base, m) = &callee.kind
    {
        out.push((m.as_str(), args.as_slice(), cur));
        cur = base;
    }
    out
}

/// Whether a call's callee is a member of `program.methods`, as in
/// `program.methods.withdraw(amount)`.
fn is_methods_call(call: &Expr) -> bool {
    matches!(&call.kind, ExprKind::Call { callee, .. }
        if matches!(&callee.kind, ExprKind::Member(base, _)
            if matches!(&base.kind, ExprKind::Member(_, m) if m == "methods")))
}

/// An Anchor request builder sent with `.rpc()` (TypeScript) or `.send()`
/// (anchor-client):
/// `program.methods.withdraw(a).accounts({..}).signers([admin]).rpc()`,
/// `program.request().accounts(..).args(..).signer(&admin).send()`.
fn anchor_builder<'p>(c: &Call<'p>) -> Option<Event<'p>> {
    let terminal = matches!(
        (c.name.as_str(), c.args.len()),
        ("rpc" | "rpc_and_keys", 0 | 1) | ("send", 0) | ("send_with_spinner_and_config", 1)
    );
    if !terminal {
        return None;
    }
    let links = chain(c.receiver?);
    let typescript = links.iter().any(|(_, _, call)| is_methods_call(call));
    let rust = links
        .iter()
        .any(|(m, args, _)| *m == "request" && args.is_empty());
    if !typescript && !rust {
        return None;
    }
    let mut signers = Vec::new();
    let mut instructions = Vec::new();
    let mut accounts = None;
    for (method, args, call) in links {
        match (method, args) {
            ("signers" | "signer", [s]) => signers.push(s),
            ("accounts" | "accounts_partial" | "accounts_strict", [a]) => accounts = Some(a),
            ("args" | "instruction" | "pre_instructions" | "post_instructions", [ix]) => {
                instructions.push(ix)
            }
            _ if is_methods_call(call) => instructions.push(call),
            _ => {}
        }
    }
    signers.reverse();
    instructions.reverse();
    Some(Event::Transaction {
        signers,
        implicit: Some(if typescript {
            PROVIDER_WALLET
        } else {
            CLIENT_PAYER
        }),
        instructions,
        accounts,
    })
}

/// Anchor's legacy namespace: `program.rpc.withdraw(amount, { accounts,
/// signers })`.
fn anchor_rpc_namespace<'p>(c: &Call<'p>) -> Option<Event<'p>> {
    let receiver = c.receiver?;
    if !matches!(&receiver.kind, ExprKind::Member(_, m) if m == "rpc") {
        return None;
    }
    let field = |name: &str| match c.args.last().map(|a| &a.kind) {
        Some(ExprKind::Call {
            callee, options, ..
        }) if matches!(&callee.kind, ExprKind::Type(t) if t.is_empty()) => {
            options.iter().find(|(n, _)| n == name).map(|(_, v)| v)
        }
        _ => None,
    };
    Some(Event::Transaction {
        signers: field("signers").into_iter().collect(),
        implicit: Some(PROVIDER_WALLET),
        instructions: vec![c.expr],
        accounts: field("accounts"),
    })
}

/// Whether `e` is written as a list: `&[..]`, `[..]`, `vec![..]`.
fn is_list(e: &Expr) -> bool {
    match &e.kind {
        ExprKind::Unary(op, inner) if op == "&" => is_list(inner),
        ExprKind::Array(_) => true,
        ExprKind::Call { callee, .. } => callee.as_ident() == Some("vec!"),
        _ => false,
    }
}

/// The signers of `tx.sign(..)`: Rust passes a list and a blockhash
/// (`tx.sign(&[&payer], hash)`), web3.js passes the keypairs
/// (`tx.sign(payer, attacker)`).
fn sign_signers(args: &[Expr]) -> Vec<&Expr> {
    let rust_form = args.len() == 2
        && (is_list(&args[0]) || words(&args[1].text).iter().any(|w| w.contains("hash")));
    if rust_form {
        vec![&args[0]]
    } else {
        args.iter().collect()
    }
}

/// The event a call causes, if it is a harness call.
pub fn event<'p>(c: &Call<'p>) -> Option<Event<'p>> {
    let args = c.args;
    let arg = |i: usize| args.get(i);
    let tx = |signers: Option<&'p Expr>, instructions: Option<&'p Expr>| Event::Transaction {
        signers: signers.into_iter().collect(),
        implicit: None,
        instructions: instructions.into_iter().collect(),
        accounts: None,
    };
    if !c.path.is_empty() {
        let p = c.path.as_str();
        if path_ends(p, &["Transaction", "new_signed_with_payer"]) {
            return Some(tx(arg(2), arg(0)));
        }
        if path_ends(p, &["Transaction", "new"]) && !args.is_empty()
            || path_ends(p, &["Transaction", "new_with_compiled_instructions"])
        {
            return Some(tx(arg(0), arg(1)));
        }
        if path_ends(p, &["VersionedTransaction", "try_new"]) {
            return Some(tx(arg(1), arg(0)));
        }
        // web3.js: sendAndConfirmTransaction(connection, tx, signers)
        if path_ends(p, &["send_and_confirm_transaction"]) && args.len() >= 3 {
            return Some(tx(arg(2), arg(1)));
        }
        let segs: Vec<&str> = p.rsplit("::").collect();
        let (last, owner) = (segs[0], segs.get(1).copied().unwrap_or(""));
        if owner.ends_with("RpcClient") && last.starts_with("new")
            || owner == "Connection" && last == "new"
            || owner == "Client" && matches!(last, "new" | "new_with_options")
        {
            return Some(Event::LiveRpc);
        }
        if owner == "AnchorLiteSVM" && last.starts_with("build_with_program") && args.len() >= 2 {
            return Some(Event::LoadProgram {
                id: &args[0],
                code: vec![&args[1]],
            });
        }
        if matches!(owner, "ProgramTest" | "LiteSVM" | "Mollusk")
            && matches!(last, "new" | "default")
        {
            if owner == "ProgramTest" && last == "new" && args.len() >= 2 {
                return Some(Event::LoadProgram {
                    id: &args[1],
                    code: vec![&args[0]],
                });
            }
            if owner == "Mollusk" && last == "new" && args.len() >= 2 {
                return Some(Event::LoadProgram {
                    id: &args[0],
                    code: vec![&args[1]],
                });
            }
            return Some(Event::Harness);
        }
        if matches!(
            (owner, last),
            ("AnchorProvider", "env" | "local" | "new")
                | ("BankrunProvider" | "LiteSVMProvider", "new")
                | ("", "start_anchor")
        ) {
            return Some(Event::Harness);
        }
        return None;
    }
    c.receiver?;
    if let Some(ev) = anchor_builder(c).or_else(|| anchor_rpc_namespace(c)) {
        return Some(ev);
    }
    match (c.name.as_str(), args.len()) {
        ("sign" | "partial_sign" | "try_sign" | "try_partial_sign", n) if n >= 1 => {
            Some(Event::Transaction {
                signers: sign_signers(args),
                implicit: None,
                instructions: Vec::new(),
                accounts: None,
            })
        }
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
        // anchor-litesvm: ctx.execute_instruction(ix, &[&user])
        ("execute_instruction" | "execute_instructions", 2) => Some(tx(arg(1), arg(0))),
        // web3.js: connection.sendTransaction(tx, [signers])
        ("send_transaction", 2) => Some(tx(arg(1), arg(0))),
        // Anchor: provider.sendAndConfirm(tx, [signers]), signed by the wallet
        ("send_and_confirm", 1..=3) => Some(Event::Transaction {
            signers: arg(1).into_iter().collect(),
            implicit: Some(PROVIDER_WALLET),
            instructions: vec![&args[0]],
            accounts: None,
        }),
        (
            "send_transaction"
            | "process_transaction"
            | "try_process_transaction"
            | "process_transaction_with_metadata"
            | "process_transaction_with_preflight"
            | "send_and_confirm_transaction"
            | "send_raw_transaction",
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
