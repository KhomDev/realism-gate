//! The rule catalog. This is the single source of truth for rule metadata:
//! `realism-gate rules`, `realism-gate explain`, the SARIF rule table and the
//! pages under `docs/rules/` are all generated from it.

use crate::finding::Severity;

#[derive(Debug, Clone, Copy)]
pub struct RuleInfo {
    pub id: &'static str,
    pub slug: &'static str,
    pub title: &'static str,
    /// The highest severity the rule can raise.
    pub severity: Severity,
    /// How the severity is chosen, in one line.
    pub severity_note: &'static str,
    pub summary: &'static str,
    pub why: &'static str,
    pub fix: &'static str,
    pub bad: &'static str,
    pub good: &'static str,
    pub false_positives: &'static [&'static str],
    /// How the rule applies to Solana tests, in Rust and Anchor TypeScript.
    pub solana: SolanaNotes,
    /// How the rule applies to Sui and Aptos Move tests.
    pub move_lang: MoveNotes,
}

/// The source language a finding was raised in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lang {
    Solidity,
    Rust,
    TypeScript,
    Move,
}

/// How a rule applies to Sui and Aptos Move tests.
#[derive(Debug, Clone, Copy)]
pub enum MoveNotes {
    /// Checked. One example in Move, shared by both chains: the two toolkits
    /// spell the same ideas (`test_scenario` / `#[test(signer)]`, a test-only
    /// mint).
    Checked {
        summary: &'static str,
        fix: &'static str,
        bad: &'static str,
        good: &'static str,
        false_positives: &'static [&'static str],
    },
    /// Not checked in Move tests, and why.
    NotChecked(&'static str),
}

/// Shared reason for rules whose evidence does not exist in a Move test.
const MOVE_NOT_THIS: &str = "Move unit tests have no Foundry cheatcode and this rule reads one, or it reads realism.yaml on its own. Privileged signers, admin caps, test-only mints, missing assertions and missing baselines are the Move checks.";

/// How a rule applies to Solana tests. Rust and TypeScript tests are read
/// into one model, so a rule applies to both or to neither.
#[derive(Debug, Clone, Copy)]
pub enum SolanaNotes {
    /// Checked. The Solana form of the summary and the fix, and examples in
    /// Rust and in Anchor TypeScript.
    Checked {
        summary: &'static str,
        fix: &'static str,
        rust_bad: &'static str,
        rust_good: &'static str,
        ts_bad: &'static str,
        ts_good: &'static str,
        false_positives: &'static [&'static str],
    },
    /// Not checked in Solana tests, and why.
    NotChecked(&'static str),
    /// The rule reads `realism.yaml`, so it is the same in both languages.
    Declaration,
}

impl RuleInfo {
    /// The fix that applies to a finding raised in `lang`.
    pub fn fix_for(&self, lang: Lang) -> &'static str {
        match lang {
            Lang::Rust | Lang::TypeScript => match self.solana {
                SolanaNotes::Checked { fix, .. } => fix,
                _ => self.fix,
            },
            Lang::Move => match self.move_lang {
                MoveNotes::Checked { fix, .. } => fix,
                MoveNotes::NotChecked(_) => self.fix,
            },
            Lang::Solidity => self.fix,
        }
    }

    /// Whether the rule applies to Solana tests, directly or through the
    /// declaration.
    pub fn checks_solana(&self) -> bool {
        !matches!(self.solana, SolanaNotes::NotChecked(_))
    }

    /// Whether the rule applies to Move tests.
    pub fn checks_move(&self) -> bool {
        matches!(self.move_lang, MoveNotes::Checked { .. })
            || matches!(self.solana, SolanaNotes::Declaration)
    }
}

pub const RULES: &[RuleInfo] = &[
    RuleInfo {
        id: "RG01",
        slug: "attacker-holds-privilege",
        title: "The attacker holds a privileged role",
        severity: Severity::Error,
        severity_note: "always error",
        summary: "The harness grants the attacker identity a role, or makes the attacker the address a privileged getter returns.",
        why: "A finding is only as strong as its weakest assumption about the attacker. If the harness grants the attacker a role (`grantRole(KEEPER, attacker)`, `transferOwnership(attacker)`, `setOperator(attacker)`), or assigns the attacker from `vault.owner()`, the PoC proves what that role can do. Programs almost always exclude privileged-role findings, and a triager reads the grant line first.",
        fix: "Run the attack from an address that holds no role. If the bug really needs a role, the finding is about role compromise or an unknowing admin action, and the report must argue that explicitly.",
        bad: "vault.grantRole(vault.KEEPER_ROLE(), attacker);\nvm.prank(attacker);\nvault.harvest(maliciousRoute);",
        good: "vm.prank(attacker); // holds no role\nvault.deposit(1e18, attacker);",
        false_positives: &[
            "A role that is permissionless in the real deployment (anyone may register as a keeper). Say so in an `allow` comment with the registration path, or declare the attacker's capability in `realism.yaml`.",
            "A grant to a variable the name heuristic reads as the attacker but is not. Declare the real attacker under `attacker.identities` in `realism.yaml`; a declaration replaces the name heuristic.",
        ],
        move_lang: MoveNotes::Checked {
            summary: "The attacker ends the test holding an admin or treasury cap (`take_from_address<AdminCap>` while the scenario sender is the attacker, or `public_transfer` of that cap to the attacker), or a `#[test]` signer named for the attacker is bound to `@0x1` or `@aptos_framework`.",
            fix: "Leave the cap with the admin signer. If the bug really needs the cap, the finding is about cap compromise and the report has to say so.",
            bad: "#[test]\nfun test_attacker_takes_the_cap() {\n    let attacker = @0xA11CE;\n    let mut scenario = test_scenario::begin(@0xAD);\n    test_scenario::next_tx(&mut scenario, @0xAD);\n    test_scenario::next_tx(&mut scenario, attacker);\n    let cap = test_scenario::take_from_address<AdminCap>(&scenario, @0xAD);\n    transfer::public_transfer(cap, attacker);\n    assert!(true, 0);\n}",
            good: "#[test]\nfun test_attacker_has_no_cap() {\n    let attacker = @0xA11CE;\n    let mut scenario = test_scenario::begin(@0xAD);\n    test_scenario::next_tx(&mut scenario, @0xAD);\n    init(test_scenario::ctx(&mut scenario));\n    test_scenario::next_tx(&mut scenario, attacker);\n    vault::deposit(test_scenario::ctx(&mut scenario));\n    let before = coin::value(&attacker_coin);\n    assert!(coin::value(&attacker_coin) >= before, 0);\n}",
            false_positives: &[
                "A cap the attacker is allowed to create for their own object, whose type happens to end in `Cap`. The rule matches `AdminCap`, `TreasuryCap` and `MintCap` only.",
            ],
        },
        solana: SolanaNotes::Checked {
            summary: "The harness makes the attacker's keypair a role holder: it is set as `admin` or `authority` in an instruction that initializes or reconfigures the program (a struct field in Rust, a field of `.accounts({ .. })` in Anchor TypeScript), given a role with `set_authority` or a `set_admin`-style call, or it is a role holder's keypair under another name.",
            fix: "Sign the attack with a keypair that holds no role. If the bug really needs a role, the finding is about role compromise or an unknowing admin action, and the report must argue that explicitly.",
            rust_bad: "let init = instruction::Initialize { admin: attacker.pubkey() };\nsend(&mut svm, &init, &[&attacker]);\nsend(&mut svm, &instruction::SetFee { bps: 10_000 }, &[&attacker]);",
            rust_good: "let init = instruction::Initialize { admin: admin.pubkey() };\nsend(&mut svm, &init, &[&admin]);\nsend(&mut svm, &instruction::Deposit { amount: 1 }, &[&attacker]);",
            ts_bad: "await program.methods.initialize().accounts({ admin: attacker.publicKey }).rpc();\nawait program.methods.setFee(new BN(10_000)).accounts({ admin: attacker.publicKey }).signers([attacker]).rpc();",
            ts_good: "await program.methods.initialize().accounts({ admin: admin.publicKey }).signers([admin]).rpc();\nawait program.methods.deposit(new BN(1)).accounts({ user: attacker.publicKey }).signers([attacker]).rpc();",
            false_positives: &[
                "A role field that names the attacker's own account rather than a protocol role (the `authority` of a position the attacker opens). Role fields are read only in structs whose name says they initialize or reconfigure (`Initialize`, `UpdateConfig`); rename the field or add an `allow` comment.",
                "A role that is permissionless in the real deployment. Say so in an `allow` comment with the registration path, or declare the attacker in `realism.yaml`.",
            ],
        },
    },
    RuleInfo {
        id: "RG02",
        slug: "privileged-step-in-exploit",
        title: "A step in the exploit runs as a privileged address",
        severity: Severity::Warn,
        severity_note: "warn; silenced by a realism.yaml `privileged:` entry",
        summary: "The test body pranks as an address that reads as a role (owner, admin, keeper, ...) or impersonates a protocol contract.",
        why: "`vm.prank(admin)` in `setUp()` is configuration. The same line in the test body is a step of the exploit that the attacker cannot sign. Sometimes that is legitimate: a keeper harvests on a schedule, governance executes a passed proposal. Then the claim depends on that role acting, and the report has to say why the attacker can rely on it. Impersonating a protocol contract (`vm.prank(address(pool))`) is never something an attacker can do.",
        fix: "Move configuration into `setUp()`. If the exploit genuinely depends on a routine privileged action, declare it in `realism.yaml` under `privileged:` with the role and the reason the attacker can count on it, and make the report say the same.",
        bad: "function test_drain() public {\n    vm.prank(admin);\n    vault.setOracle(address(fakeOracle));\n    vm.prank(attacker);\n    vault.borrow(1_000e18);\n}",
        good: "function test_drain() public {\n    // The oracle is the live one; nothing privileged happens here.\n    vm.prank(attacker);\n    vault.borrow(1_000e18);\n}",
        false_positives: &[
            "A role word in an ordinary user's name (`managerOfFunds` for a user account). Rename the variable or declare it under `victim.identities`.",
            "`owner` used for an ERC-4626 share owner. Prefer `shareOwner` or `holder`, which the tool reads as a user.",
        ],
        move_lang: MoveNotes::Checked {
            summary: "After setup, the scenario's sender goes back to a privileged name (`next_tx(..., admin)`), or an Aptos `#[test]` signer named `admin` (or bound to `@0x1`) is passed to a call that is not `init` or `setup`.",
            fix: "Keep the privileged signer in the setup transaction. The attacker's later transaction should be the one that breaks the invariant.",
            bad: "#[test]\nfun test_admin_acts_in_the_exploit() {\n    let admin = @0xAD;\n    let attacker = @0xA11CE;\n    let mut scenario = test_scenario::begin(admin);\n    test_scenario::next_tx(&mut scenario, attacker);\n    test_scenario::next_tx(&mut scenario, admin);\n    vault::set_fee(1, test_scenario::ctx(&mut scenario));\n    assert!(coin::value(&v) == 0, 0);\n}",
            good: "#[test]\nfun test_only_the_attacker_acts() {\n    let admin = @0xAD;\n    let attacker = @0xA11CE;\n    let mut scenario = test_scenario::begin(admin);\n    test_scenario::next_tx(&mut scenario, attacker);\n    vault::deposit(test_scenario::ctx(&mut scenario));\n    let before = coin::value(&attacker_coin);\n    assert!(coin::value(&attacker_coin) >= before, 0);\n}",
            false_positives: &[
                "A second `next_tx(admin)` that is still setup (funding, creating a shared object) and is not named like it. Put that work in the first transaction or in a function named `setup`.",
            ],
        },
        solana: SolanaNotes::Checked {
            summary: "A transaction in the exploit is signed by a keypair that reads as a role (admin, authority, governance, keeper, ...) or that the harness gave a role.",
            fix: "Move configuration before the attacker's first transaction. If the exploit genuinely depends on a routine privileged action, declare it in `realism.yaml` under `privileged:` with the role and the reason the attacker can count on it, and make the report say the same.",
            rust_bad: "send(&mut svm, &set_oracle_ix(&fake_oracle), &[&admin]);\nsend(&mut svm, &borrow_ix(&attacker.pubkey(), 1_000), &[&attacker]);",
            rust_good: "// The oracle is the live one; nothing privileged happens here.\nsend(&mut svm, &borrow_ix(&attacker.pubkey(), 1_000), &[&attacker]);",
            ts_bad: "await program.methods.setOracle(fakeOracle.publicKey).accounts({ admin: admin.publicKey }).signers([admin]).rpc();\nawait program.methods.borrow(new BN(1_000)).accounts({ borrower: attacker.publicKey }).signers([attacker]).rpc();",
            ts_good: "// The oracle is the live one; nothing privileged happens here.\nawait program.methods.borrow(new BN(1_000)).accounts({ borrower: attacker.publicKey }).signers([attacker]).rpc();",
            false_positives: &[
                "`authority` used for a user's own authority under a bare name. Prefer `user_authority` or the person's name, which the tool reads as a user.",
                "A keypair named for a role the program never checks. Rename it or add an `allow` comment.",
            ],
        },
    },
    RuleInfo {
        id: "RG03",
        slug: "deployer-is-attacker",
        title: "The exploit runs as the contract that deployed the protocol",
        severity: Severity::Warn,
        severity_note: "warn when a call is named like an access-controlled function; note for any other call the deployer makes",
        summary: "The test contract deploys the code under review and then calls it without changing sender. A warning means one of those calls is named like an access-controlled function. A note means the calls are ordinary and this is the local-deployment pattern.",
        why: "Constructors commonly give the deployer ownership or every role. A test that deploys with `new Vault()` and then calls `vault.sweep()` from the test contract has proved that the deployer can sweep. A test that deploys locally and then calls `deposit` or `hash` has not: that is how almost every local harness is written, and reporting it as a warning buried the cases that matter. The warning is kept for the call whose name says it is gated. The note remains so the pattern is visible without failing a run that fails on warnings.",
        fix: "Deploy and configure as one address, attack as another: `vm.startPrank(attacker)` before the exploit steps, or transfer ownership to a dedicated owner in `setUp()`. For a note, do that if any of the calls is in fact gated under a name the tool does not recognise, or silence it with an `allow` comment.",
        bad: "function setUp() public { vault = new Vault(); }\nfunction test_sweep() public {\n    vault.sweep(address(token)); // runs as the deployer\n}",
        good: "function setUp() public { vault = new Vault(); }\nfunction test_sweep() public {\n    vm.prank(attacker);\n    vault.sweep(address(token));\n}",
        false_positives: &[
            "A function that is gated but is not named like one (`harvest`, `skim`, `poke`). The finding is a note, and the gate is missed. Rename nothing; an `allow` comment is the wrong tool here, because the note is already the low severity. Say so in the report.",
            "A function that is named like a setter and is permissionless. The warning is wrong. Silence it with an `allow` comment that says the function is ungated.",
            "A constructor that hands ownership to an argument the tool does not read as privileged. Pass a variable named for the role (`owner`, `admin`) or the rule will not see the handover and will keep the deployer as the owner.",
        ],
        move_lang: MoveNotes::NotChecked(MOVE_NOT_THIS),
        solana: SolanaNotes::Checked {
            summary: "The exploit's transactions are signed only by the keys that signed the setup, usually the harness's `payer` or Anchor's `provider.wallet` (which signs every `.rpc()`), and by no separate attacker keypair.",
            fix: "Set up as one keypair, attack as another: create an `attacker` keypair, fund it, and have it sign the exploit.",
            rust_bad: "let (mut banks, payer, _) = program_test.start().await;\nprocess(&mut banks, &[init_pool_ix(&payer.pubkey())], &[&payer]).await;\nprocess(&mut banks, &[sweep_ix(&payer.pubkey())], &[&payer]).await;",
            rust_good: "process(&mut banks, &[init_pool_ix(&payer.pubkey())], &[&payer]).await;\nlet attacker = Keypair::new();\nprocess(&mut banks, &[sweep_ix(&attacker.pubkey())], &[&payer, &attacker]).await;",
            ts_bad: "await program.methods.initPool().accounts({ payer: provider.wallet.publicKey }).rpc();\nawait program.methods.sweep().accounts({ recipient: provider.wallet.publicKey }).rpc();",
            ts_good: "await program.methods.initPool().accounts({ payer: provider.wallet.publicKey }).rpc();\nawait program.methods.sweep().accounts({ recipient: attacker.publicKey }).signers([attacker]).rpc();",
            false_positives: &[
                "A program with no access control, where the setup key gains nothing. The finding is a note when the instruction is not named like a gated call, and a warning when it is (`sweep`, `set_fee`). An `allow` comment is how to silence the warning.",
                "Setup the tool does not recognise as setup (no `init`- or `create`-style instruction name, no helper named `setup` or similar, no `before` hook or test titled for setup), which moves the start of the exploit earlier. Put the setup in a function named `setup`, or in a `before` hook.",
            ],
        },
    },
    RuleInfo {
        id: "RG04",
        slug: "storage-write",
        title: "The PoC writes contract storage directly",
        severity: Severity::Error,
        severity_note: "error in the exploit; warn in setUp; one step lower when the slot looks like an attacker or user balance",
        summary: "`vm.store` sets a storage slot of a contract under review.",
        why: "`vm.store` creates state without going through the code that is supposed to guard it. A PoC that writes the broken state has assumed the bug instead of reaching it. In `setUp()` it can be legitimate (seeding a mapping a fork cannot reach), which is why it is a warning there; in the exploit it is never something an attacker can do.",
        fix: "Reach the state through the protocol's own entry points. If a fork already holds the state, pin the block where it does. If the slot is a token balance for funding, use `deal` on the attacker instead.",
        bad: "vm.store(address(vault), bytes32(uint256(7)), bytes32(uint256(0))); // zero the debt",
        good: "vm.prank(attacker);\nvault.repay(0); // the bug: repay(0) clears the debt",
        false_positives: &[
            "Writing a token balance slot of the attacker (manual funding for a token `deal` cannot handle). The rule lowers the severity when the slot expression names an attacker or user; add an `allow` comment saying it is funding.",
        ],
        move_lang: MoveNotes::NotChecked(MOVE_NOT_THIS),
        solana: SolanaNotes::Checked {
            summary: "`set_account` or `add_account` (`setAccount` in TypeScript, or an account handed to bankrun's `startAnchor`) writes an account of the program: a PDA, an account at a fixed address or named for protocol state, or one the test creates with the program as its owner.",
            fix: "Reach the state through the program's own instructions. If the state exists on a live cluster, dump the account at a known slot and load it from the file. To fund the attacker, write the attacker's own accounts instead.",
            rust_bad: "let mut pool = svm.get_account(&pool_pda).unwrap();\npool.data[8..16].copy_from_slice(&0u64.to_le_bytes()); // zero the debt\nsvm.set_account(pool_pda, pool).unwrap();",
            rust_good: "// The bug: repaying 0 clears the debt.\nsend(&mut svm, &repay_ix(&attacker.pubkey(), 0), &[&attacker]);",
            ts_bad: "const pool = svm.getAccount(poolPda)!;\npool.data.fill(0, 8, 16); // zero the debt\nsvm.setAccount(poolPda, pool);",
            ts_good: "// The bug: repaying 0 clears the debt.\nawait program.methods.repay(new BN(0)).accounts({ borrower: attacker.publicKey }).signers([attacker]).rpc();",
            false_positives: &[
                "An account the test creates for itself under a protocol-sounding name, such as the attacker's own pool on a permissionless program. Rename it or add an `allow` comment.",
                "State copied from a live cluster through a helper the tool does not recognise as a copy. Load it from a dump file, or add an `allow` comment naming the source.",
            ],
        },
    },
    RuleInfo {
        id: "RG05",
        slug: "code-replacement",
        title: "The PoC replaces deployed code",
        severity: Severity::Warn,
        severity_note: "warn; capped below error since 0.2.0 because its measured precision is under 80% (docs/precision.md)",
        summary: "`vm.etch` or `deployCodeTo` puts different bytecode at an address the protocol uses.",
        why: "Replacing a dependency's code means the PoC no longer runs against the deployed system. Whatever the replacement does, the attacker cannot make the real contract do it. In `setUp()` this is sometimes how a harness installs a test double at a fixed address, which is why it is a warning there; the report then has to show the real dependency behaves the same way.",
        fix: "Run against the real dependency on a pinned fork. If a double is unavoidable, explain in the report why the real contract can return the same values, and cite where.",
        bad: "vm.etch(ORACLE, address(new StaleOracle()).code);",
        good: "vm.createSelectFork(rpc, BLOCK_WHERE_ORACLE_IS_STALE);",
        false_positives: &[
            "Etching a precompile or a chain-specific system contract the local EVM lacks. Add an `allow` comment naming it.",
            "Recreating the attacker's own historical contract at its original address, as post-incident reproductions do. Name the address for what it is (`ATTACKER_CONTRACT`, `exploitHelper`) and the tool treats it as the attacker's.",
        ],
        move_lang: MoveNotes::NotChecked(MOVE_NOT_THIS),
        solana: SolanaNotes::Checked {
            summary: "A program whose name says it is a test double (mock, fake, stub, dummy) is loaded at the address of a real program: a crate's `ID` or a fixed public key (`pubkey!(..)`, `new PublicKey(\"..\")`). In TypeScript this covers LiteSVM's `addProgramFromFile` and the programs handed to bankrun's `startAnchor`.",
            fix: "Run against the real program, loaded from its deployed binary. If a double is unavoidable, explain in the report why the real program can return the same values, and cite where.",
            rust_bad: "svm.add_program(oracle::ID, include_bytes!(\"fixtures/mock_oracle.so\"));",
            rust_good: "svm.add_program_from_file(oracle::ID, \"fixtures/oracle.so\").unwrap();",
            ts_bad: "const PYTH = new PublicKey(\"FsJ3A3u2vn5cTVofAjvy6y5kwABJAqYWpe4975bi2epH\");\nsvm.addProgramFromFile(PYTH, \"fixtures/mock_pyth.so\");",
            ts_good: "const PYTH = new PublicKey(\"FsJ3A3u2vn5cTVofAjvy6y5kwABJAqYWpe4975bi2epH\");\nsvm.addProgramFromFile(PYTH, \"fixtures/pyth.so\");",
            false_positives: &[
                "A double for a program that does not exist on the target cluster. Add an `allow` comment naming it.",
            ],
        },
    },
    RuleInfo {
        id: "RG06",
        slug: "balance-injection",
        title: "The PoC sets a protocol contract's balance",
        severity: Severity::Error,
        severity_note: "error in the exploit; warn in setUp",
        summary: "`deal` or `vm.deal` writes tokens or ether into a contract under review.",
        why: "`deal` does not transfer: it overwrites the balance. Pointed at a protocol contract it does two wrong things at once. It creates value nobody paid for, and it erases whatever balance the contract had, so the contract's internal accounting (reserves, total assets, debt) never sees the change. A donation attack done with `deal` instead of `transfer` is not a donation attack.",
        fix: "Fund the attacker with `deal`, then `transfer` from the attacker to the contract. To seed liquidity, go through the protocol's own deposit or mint path.",
        bad: "deal(address(token), address(vault), 1_000e18); // \"donation\"",
        good: "deal(address(token), attacker, 1_000e18);\nvm.prank(attacker);\ntoken.transfer(address(vault), 1_000e18);",
        false_positives: &[
            "A contract variable the tool classifies as protocol that is really the PoC's own helper defined elsewhere. Helpers defined in the same file as the test are recognised automatically.",
        ],
        move_lang: MoveNotes::Checked {
            summary: "`coin::mint_for_testing`, `mint_for_test` or `mint_to_for_testing` creates coins the harness then transfers to the attacker. Those coins were not minted by the protocol's treasury.",
            fix: "Mint through the protocol's own mint, with the treasury cap held by whoever holds it in deployment. Funding the attacker with `mint_for_testing` is the Move form of `deal`; say so in an `allow` comment when that is all it is.",
            bad: "#[test]\nfun test_mint_for_testing_to_attacker() {\n    let attacker = @0xA11CE;\n    let c = coin::mint_for_testing<SUI>(1000);\n    transfer::public_transfer(c, attacker);\n    let before = coin::value(&attacker_coin);\n    assert!(coin::value(&attacker_coin) == before, 0);\n}",
            good: "#[test]\nfun test_mint_stays_with_the_vault() {\n    let c = coin::mint_for_testing<SUI>(1000);\n    transfer::public_transfer(c, @0xCAFE);\n    vault::deposit();\n    let before = coin::value(&vault_coin);\n    assert!(coin::value(&vault_coin) == before, 0);\n}",
            false_positives: &[
                "The mint is the attacker's starting capital and the profit is a delta above it. The rule cannot see that. An `allow` comment saying the coins are the budget, not the profit, is the honest silence.",
            ],
        },
        solana: SolanaNotes::Checked {
            summary: "`set_account` or `add_account` (`setAccount` in TypeScript, or an account handed to bankrun's `startAnchor`) sets the lamports or the token balance of an account of the program, such as a vault's token account.",
            fix: "Fund the attacker's own accounts, then transfer from the attacker to the program's account. To seed liquidity, go through the program's own deposit instruction.",
            rust_bad: "svm.set_account(vault_tokens, token_account(&mint, &vault_pda, 1_000_000)).unwrap();",
            rust_good: "svm.set_account(attacker_tokens, token_account(&mint, &attacker.pubkey(), 1_000_000)).unwrap();\nsend(&mut svm, &transfer_ix(&attacker_tokens, &vault_tokens, 1_000_000), &[&attacker]);",
            ts_bad: "svm.setAccount(vaultTokens, { lamports: 2_039_280, data: tokenAccountData(USDC, vaultPda, 1_000_000n), owner: TOKEN_PROGRAM_ID, executable: false });",
            ts_good: "svm.setAccount(attackerTokens, { lamports: 2_039_280, data: tokenAccountData(USDC, attacker.publicKey, 1_000_000n), owner: TOKEN_PROGRAM_ID, executable: false });\nawait program.methods.deposit(new BN(1_000_000)).accounts({ from: attackerTokens, vault: vaultTokens }).signers([attacker]).rpc();",
            false_positives: &[
                "Seeding a pool the test creates itself on a permissionless program, where the attacker could do the same with their own funds. An `allow` comment saying so documents it.",
            ],
        },
    },
    RuleInfo {
        id: "RG07",
        slug: "mocked-call",
        title: "The PoC mocks a dependency's return value",
        severity: Severity::Warn,
        severity_note: "warn",
        summary: "`vm.mockCall` or `vm.mockCallRevert` makes a contract return a value its code would not.",
        why: "A mocked return is an assumption. If the finding's impact flows through that value (a price, a balance, a success flag), the PoC shows what happens if the dependency misbehaves, not that an attacker can make it misbehave. That is a different, usually much weaker, claim.",
        fix: "Demonstrate how the attacker produces that return value on the real dependency, or restate the finding as conditional on the dependency's failure and price it accordingly.",
        bad: "vm.mockCall(address(oracle), abi.encodeWithSelector(IOracle.price.selector), abi.encode(1));",
        good: "// Move the real price with the attacker's own trade:\npool.swap(attacker, true, 5_000e18, MIN_SQRT_PRICE, \"\");",
        false_positives: &[
            "Mocking something irrelevant to the claim, such as a gas-price oracle in a test that measures a rounding error. Add an `allow` comment saying why it does not touch the impact.",
        ],
        move_lang: MoveNotes::NotChecked(MOVE_NOT_THIS),
        solana: SolanaNotes::NotChecked(
            "Solana test harnesses have no cheat that makes a program answer with a value its code would not produce. Loading a test double in a real program's place is RG05.",
        ),
    },
    RuleInfo {
        id: "RG08",
        slug: "no-assertion",
        title: "The test asserts nothing",
        severity: Severity::Error,
        severity_note: "error; warn when the test logs values it never checks",
        summary: "No `assert*`, `require`, `vm.expect*` or `fail` runs in the test, its helpers or its callbacks.",
        why: "A test with no assertion passes whatever happens. Logging the attacker's balance is not a check: a reviewer has to read the numbers and trust that they mean what the report says. An assertion turns the claim into something the test enforces.",
        fix: "Assert the impact: the attacker's gain against a baseline, the victim's loss, or the broken invariant.",
        bad: "emit log_named_uint(\"profit\", token.balanceOf(address(this)));",
        good: "uint256 before = token.balanceOf(attacker);\n_attack();\nassertGt(token.balanceOf(attacker), before + 1_000e18);",
        false_positives: &[
            "Assertions inside a helper contract the test calls externally. The tool inlines the test contract's own functions, not other contracts'. Move the check into the test.",
        ],
        move_lang: MoveNotes::Checked {
            summary: "The test function contains no `assert!`. `#[expected_failure]` is not this rule; a test whose only pass condition is an abort is RG18.",
            fix: "Assert the impact: a coin value against a baseline, or the broken invariant.",
            bad: "#[test]\nfun test_withdraws_and_prints() {\n    let attacker = @0xA11CE;\n    test_scenario::next_tx(&mut scenario, attacker);\n    vault::withdraw();\n}",
            good: "#[test]\nfun test_withdraws() {\n    let attacker = @0xA11CE;\n    test_scenario::next_tx(&mut scenario, attacker);\n    let before = coin::value(&attacker_coin);\n    vault::withdraw();\n    assert!(coin::value(&attacker_coin) > before, 0);\n}",
            false_positives: &[
                "An assertion in a helper the test calls. Only the test function's own body is read. Move the check into the test.",
            ],
        },
        solana: SolanaNotes::Checked {
            summary: "No assertion runs in the exploit: in Rust `assert!`, `assert_eq!`, `assert_ne!`, `unwrap_err()`, `if .. { panic!() }` or a validating Mollusk check, and the test is not `#[should_panic]`; in TypeScript chai's `assert` and `expect`, node's `assert`, jest's `expect`, or `if (..) throw`. A transaction that succeeds (`.unwrap()`, `await ..rpc()`) does not count: it only shows the transaction did not fail.",
            fix: "Assert the impact: the attacker's gain against a baseline, the victim's loss, or the broken invariant.",
            rust_bad: "svm.send_transaction(exploit_tx).unwrap();\nprintln!(\"profit: {}\", svm.get_balance(&attacker.pubkey()).unwrap());",
            rust_good: "let before = svm.get_balance(&attacker.pubkey()).unwrap();\nsvm.send_transaction(exploit_tx).unwrap();\nassert!(svm.get_balance(&attacker.pubkey()).unwrap() > before + LAMPORTS_PER_SOL);",
            ts_bad: "await program.methods.withdraw(new BN(100)).accounts({ recipient: attacker.publicKey }).signers([attacker]).rpc();\nconsole.log(\"profit\", await provider.connection.getBalance(attacker.publicKey));",
            ts_good: "const balanceBefore = await provider.connection.getBalance(attacker.publicKey);\nawait program.methods.withdraw(new BN(100)).accounts({ recipient: attacker.publicKey }).signers([attacker]).rpc();\nexpect(await provider.connection.getBalance(attacker.publicKey)).to.be.greaterThan(balanceBefore + LAMPORTS_PER_SOL);",
            false_positives: &[
                "Assertions inside a helper the tool does not read. Only the test's own file and test files in the same language under `tests/` are inlined; move the check into the test.",
            ],
        },
    },
    RuleInfo {
        id: "RG09",
        slug: "tautological-assertion",
        title: "The assertion cannot fail",
        severity: Severity::Error,
        severity_note: "error",
        summary: "An assertion compares two constants, compares an expression with itself, or asserts a literal `true`.",
        why: "`assertEq(profit, 100e18)` where `profit = 100e18` two lines earlier is a check of the test's own arithmetic. It passes regardless of what the protocol did, so it lends the PoC a credibility it has not earned.",
        fix: "Assert on state read from the system after the attack, not on values the test wrote itself.",
        bad: "uint256 stolen = 100e18;\nassertEq(stolen, 100e18);",
        good: "assertEq(token.balanceOf(attacker) - before, 100e18);",
        false_positives: &[
            "A deliberate sanity check of a test constant. Keep it if you want it, but it is not evidence; an `allow` comment documents that.",
        ],
        move_lang: MoveNotes::NotChecked(MOVE_NOT_THIS),
        solana: SolanaNotes::Checked {
            summary: "An assertion (`assert!`, `assert_eq!`, `assert.equal`, `expect(..).to.equal`, ...) compares two constants, compares an expression with itself, or asserts a literal `true`.",
            fix: "Assert on state read from the program's accounts after the attack, not on values the test wrote itself.",
            rust_bad: "let stolen = 100 * LAMPORTS_PER_SOL;\nassert_eq!(stolen, 100 * LAMPORTS_PER_SOL);",
            rust_good: "assert_eq!(svm.get_balance(&attacker.pubkey()).unwrap() - before, 100 * LAMPORTS_PER_SOL);",
            ts_bad: "const stolen = 100 * LAMPORTS_PER_SOL;\nassert.equal(stolen, 100 * LAMPORTS_PER_SOL);",
            ts_good: "assert.equal((await provider.connection.getBalance(attacker.publicKey)) - balanceBefore, 100 * LAMPORTS_PER_SOL);",
            false_positives: &[
                "A deliberate sanity check of a test constant. Keep it if you want it, but it is not evidence; an `allow` comment documents that.",
            ],
        },
    },
    RuleInfo {
        id: "RG10",
        slug: "no-baseline",
        title: "The impact is measured without a before-value",
        severity: Severity::Warn,
        severity_note: "warn",
        summary: "A directional assertion (greater than, less than) checks a balance or state read that happens once, after the attack. Nothing records what it was before.",
        why: "`assertGt(token.balanceOf(attacker), 0)` passes if the attacker already had tokens: from `deal`, from a fork's existing holdings, from a `setUp()` transfer. A gain is a difference. Without the earlier read, the PoC cannot tell the exploit's effect from the starting state, and neither can the reader.",
        fix: "Read the same quantity before and after, and assert on the difference.",
        bad: "_attack();\nassertGt(token.balanceOf(attacker), 0);",
        good: "uint256 before = token.balanceOf(attacker);\n_attack();\nassertGt(token.balanceOf(attacker), before);",
        false_positives: &[
            "A directional claim that is meaningful on its own, such as \"the attacker ends with more than the vault's whole TVL\". Exact-value assertions (`assertEq`), upper bounds of zero (\"drained\") and `require` guards are already exempt; for anything else, an `allow` comment saying why the absolute value suffices.",
            "A baseline taken through a differently written expression (a local alias of the token). Read it the same way both times.",
        ],
        move_lang: MoveNotes::Checked {
            summary: "`assert!(coin::value(...) > 0)` or `assert!(balance > 0)` with no earlier `coin::value` or `balance` read stored as a before-value.",
            fix: "Read the same quantity before and after, and assert on the difference.",
            bad: "#[test]\nfun test_profit_without_a_before() {\n    let attacker = @0xA11CE;\n    test_scenario::next_tx(&mut scenario, attacker);\n    vault::withdraw();\n    assert!(coin::value(&attacker_coin) > 0, 0);\n}",
            good: "#[test]\nfun test_profit_against_a_before() {\n    let attacker = @0xA11CE;\n    test_scenario::next_tx(&mut scenario, attacker);\n    let before = coin::value(&attacker_coin);\n    vault::withdraw();\n    assert!(coin::value(&attacker_coin) > before, 0);\n}",
            false_positives: &[
                "A before-value read through a different helper than the assertion uses. Read it the same way both times, or add an `allow` comment.",
            ],
        },
        solana: SolanaNotes::Checked {
            summary: "A directional assertion (`assert!(a > b)`, `expect(a).to.be.greaterThan(b)`, `assert.isAbove`) checks a balance read once, after the attack: `get_balance` / `getBalance`, a `.lamports` or `.amount` field, or a helper whose name says balance or amount.",
            fix: "Read the same quantity before and after, and assert on the difference.",
            rust_bad: "svm.send_transaction(exploit_tx).unwrap();\nassert!(token_balance(&svm, &attacker_tokens) > 0);",
            rust_good: "let before = token_balance(&svm, &attacker_tokens);\nsvm.send_transaction(exploit_tx).unwrap();\nassert!(token_balance(&svm, &attacker_tokens) > before);",
            ts_bad: "await program.methods.withdraw(new BN(100)).accounts({ recipient: attacker.publicKey }).signers([attacker]).rpc();\nconst account = await getAccount(provider.connection, attackerTokens);\nexpect(Number(account.amount)).to.be.greaterThan(0);",
            ts_good: "const balanceBefore = (await getAccount(provider.connection, attackerTokens)).amount;\nawait program.methods.withdraw(new BN(100)).accounts({ recipient: attacker.publicKey }).signers([attacker]).rpc();\nconst balanceAfter = (await getAccount(provider.connection, attackerTokens)).amount;\nexpect(Number(balanceAfter)).to.be.greaterThan(Number(balanceBefore));",
            false_positives: &[
                "A baseline read through a differently written expression (a second helper, an alias). Read it the same way both times.",
                "A directional claim that is meaningful on its own. Exact-value assertions (`assert_eq!`) and upper bounds of zero are already exempt; for anything else, an `allow` comment saying why the absolute value suffices.",
            ],
        },
    },
    RuleInfo {
        id: "RG11",
        slug: "unpinned-fork",
        title: "The fork is not pinned to a block",
        severity: Severity::Warn,
        severity_note: "warn",
        summary: "`vm.createSelectFork(url)` or `vm.createFork(url)` without a block number, and no later `vm.rollFork`.",
        why: "An unpinned fork runs against whatever the chain looks like today. Balances, prices, configuration and the bug itself can change between the day the PoC is written and the day a triager runs it. A PoC that cannot be reproduced is a PoC the triager has to take on trust.",
        fix: "Pass the block: `vm.createSelectFork(url, BLOCK)`, and say in the report why that block is representative.",
        bad: "vm.createSelectFork(vm.envString(\"RPC_URL\"));",
        good: "vm.createSelectFork(vm.envString(\"RPC_URL\"), 19_000_000);",
        false_positives: &[
            "A block pinned in `foundry.toml` (`fork_block_number`) or on the command line. The tool reads Solidity only; add an `allow` comment pointing at the config.",
        ],
        move_lang: MoveNotes::NotChecked(MOVE_NOT_THIS),
        solana: SolanaNotes::Checked {
            summary: "The test creates a client for a public cluster (an `RpcClient`, an anchor-client `Client`, a web3.js `Connection`), so the accounts it reads through it come from a live cluster as it is when the test runs.",
            fix: "Dump each account the PoC needs at a recorded slot (`solana account <ADDRESS> --output-file <FILE>`) and load it from the file, and say in the report why that slot is representative.",
            rust_bad: "let rpc = RpcClient::new(rpc_url);\nlet pool = rpc.get_account(&POOL).unwrap();\nsvm.set_account(POOL, pool).unwrap();",
            rust_good: "// Dumped at a recorded slot; the report says why that slot.\nprogram_test.add_account_with_file_data(POOL, POOL_LAMPORTS, vault::ID, \"pool.bin\");",
            ts_bad: "const mainnet = new Connection(clusterApiUrl(\"mainnet-beta\"));\nconst pool = await mainnet.getAccountInfo(POOL);\nsvm.setAccount(POOL, pool);",
            ts_good: "// Dumped at a recorded slot; the report says why that slot.\nsvm.setAccount(POOL, JSON.parse(readFileSync(\"fixtures/pool-slot-312000000.json\", \"utf8\")));",
            false_positives: &[
                "A client for a local validator whose URL comes from a variable or the environment. A literal `localhost` or `127.0.0.1` URL, `Cluster::Localnet` and the provider's own `rpcEndpoint` are recognised; otherwise add an `allow` comment.",
            ],
        },
    },
    RuleInfo {
        id: "RG12",
        slug: "attacker-in-foreign-transaction",
        title: "An attacker step is declared inside someone else's transaction",
        severity: Severity::Error,
        severity_note: "error",
        summary: "realism.yaml declares a precondition the attacker performs with `boundary: victim_transaction`.",
        why: "On the EVM a transaction has one sender. Nobody can append a call to a transaction another account signs: mempool ordering and bundles put separate transactions next to each other, never one account's call inside another's. A PoC that runs the attacker's step inside the victim's call (often with one `vm.startPrank` around both) is showing something the chain cannot produce.",
        fix: "Split the steps into separate transactions with separate senders. If the attack needs the victim's transaction to call back into attacker code, declare `callback_from_victim_call` and show where the callback comes from.",
        bad: "preconditions:\n  - step: attacker swaps inside the victim's deposit\n    performed_by: attacker\n    boundary: victim_transaction",
        good: "preconditions:\n  - step: attacker swaps before the victim's deposit\n    performed_by: attacker\n    boundary: own_transaction",
        false_positives: &[],
        move_lang: MoveNotes::NotChecked(MOVE_NOT_THIS),
        solana: SolanaNotes::Declaration,
    },
    RuleInfo {
        id: "RG13",
        slug: "unordered-same-block",
        title: "A same-block step has no ordering guarantee",
        severity: Severity::Warn,
        severity_note: "warn",
        summary: "realism.yaml declares an attacker step with `boundary: same_block` and no `ordering_guarantee`.",
        why: "Landing next to a victim's transaction in the same block needs a mechanism: a public mempool and priority fee, a builder bundle, a sequencer's ordering policy. Without one the attack works only when the ordering happens to fall the attacker's way, and the report should say how often that is.",
        fix: "State the ordering mechanism under `ordering_guarantee`, or restate the attack as one that does not need adjacency.",
        bad: "  - step: back-run the oracle update\n    performed_by: attacker\n    boundary: same_block",
        good: "  - step: back-run the oracle update\n    performed_by: attacker\n    boundary: same_block\n    ordering_guarantee: public mempool; update is a plain transaction the attacker can back-run with a bundle",
        false_positives: &[],
        move_lang: MoveNotes::NotChecked(MOVE_NOT_THIS),
        solana: SolanaNotes::Declaration,
    },
    RuleInfo {
        id: "RG14",
        slug: "dependent-precondition",
        title: "The attack depends on another party acting",
        severity: Severity::Note,
        severity_note: "note",
        summary: "realism.yaml declares a precondition performed by the victim, a privileged role or a third party.",
        why: "This is not a defect, it is a fact the severity argument has to carry. An attack that needs a victim to deposit, a keeper to harvest or governance to vote is real, but it is conditional, and triagers price conditional attacks below unconditional ones. The note keeps the dependency visible in the output.",
        fix: "Name the dependency in the report's attack requirements and argue how likely it is.",
        bad: "",
        good: "  - step: a user deposits into the empty vault\n    performed_by: victim\n    boundary: own_transaction",
        false_positives: &[],
        move_lang: MoveNotes::NotChecked(MOVE_NOT_THIS),
        solana: SolanaNotes::Declaration,
    },
    RuleInfo {
        id: "RG15",
        slug: "profit-ignores-costs",
        title: "The asserted profit is not net of a cost the test itself measures",
        severity: Severity::Warn,
        severity_note: "warn",
        summary: "An assertion of attacker profit (a value named profit, gain, extracted, proceeds or pnl) does not subtract a fee, slippage or gas cost that the same test computes, or a sibling test shows that the same flow loses money once a fee applies.",
        why: "A passing profit assertion can still be unpayable. The shape this rule exists for is a test that asserts a taker profit on a strategy with no fee, while another test in the same file asserts the taker delta is negative once the fee the protocol charges is turned on. The headline number is then the no-fee configuration, and the fee is the one a real user pays. A token balance is often already net of a pool fee; the rule fires only when the test itself introduces a cost and then ignores it.",
        fix: "Subtract the fee, the slippage and, for an ether profit, the gas from the asserted value, and assert that the net is still positive. If a sibling test shows the fee wipes the profit, the report has to be about the configuration where the fee applies, or it has to explain why that configuration is not the one that runs.",
        bad: "uint256 fee = amount * 30 / 10_000;\nuint256 profit = token.balanceOf(attacker) - before;\nassertGt(profit, 0);",
        good: "uint256 fee = amount * 30 / 10_000;\nuint256 profit = token.balanceOf(attacker) - before - fee;\nassertGt(profit, 0);",
        false_positives: &[
            "The balance read is already net of the pool's fee, and the `fee` variable is a quote the assertion does not need. Subtracting it again would double-count. Silence with an `allow` comment that says the balance is the net.",
            "A flash-loan `fee` that is repaid with `amount + fee` before the profit is read. That repayment is recognised and the rule stays silent. A repayment written differently is not, and will be flagged.",
            "Gas read with `gasleft()` next to a token profit. Gas is a different asset; the rule does not demand it be subtracted from a token balance. It does demand it of an ether `.balance` profit.",
            "The sibling test that 'concedes' the loss is about a different flow that happens to share the file and the word fee. The rule cannot tell two flows apart. Read the sibling before treating the warning as fatal.",
        ],
        move_lang: MoveNotes::NotChecked(
            "Move tests are not checked for fee-net profit. The rule reads Solidity assertions and sibling tests.",
        ),
        solana: SolanaNotes::NotChecked(
            "Solana tests are not checked for fee-net profit. A token balance on Solana is not `balanceOf`, and the rule does not yet read fee variables there.",
        ),
    },
    RuleInfo {
        id: "RG16",
        slug: "implausible-amount",
        title: "A dealt or minted amount is implausible",
        severity: Severity::Warn,
        severity_note: "warn",
        summary: "`deal`, `vm.deal` or `mint` moves at least 10^30 raw units (or `type(uint256).max`), or, on a fork pinned to a block, at least 10^27 raw units.",
        why: "An amount that large is not a balance anyone holds and not a trade anyone does. On a pinned fork it also replaces the balance the block actually had, so the profit is computed against a number the chain did not have. The tool does not read the chain, so it cannot know the real balance at that block; the bounds are fixed and stated here. A mislabeled unit (a wei amount described in a comment as a much smaller token amount) is a plausible literal and is not flagged.",
        fix: "Use an amount a single actor could hold at the pinned block, and say in the report why that size is representative. Do not `deal` over a fork balance that the test could have transferred from its real holder.",
        bad: "vm.createSelectFork(rpc, BLOCK);\ndeal(address(token), attacker, 1_000_000_000_000e18);",
        good: "vm.createSelectFork(rpc, BLOCK);\nvm.prank(WHALE);\ntoken.transfer(attacker, 1_000e18);",
        false_positives: &[
            "A token with very few decimals, or a pointed test of an overflow, where 10^27 raw units is the case under test. Say so in an `allow` comment.",
            "The bound is not the balance at the pinned block. A deal of 1000 tokens on a fork is under the bound even when that address held 1 token at the block. The rule cannot see the block.",
            "`approve(..., type(uint256).max)` is not a deal or a mint and is not flagged. An infinite approval is not an infinite balance.",
        ],
        move_lang: MoveNotes::NotChecked(
            "Move tests are not checked for implausible amounts. `mint_for_testing` into the attacker is RG06.",
        ),
        solana: SolanaNotes::NotChecked(
            "Solana tests are not checked for implausible lamport or token amounts. Writing a program account's balance is RG06.",
        ),
    },
    RuleInfo {
        id: "RG17",
        slug: "unevidenced-setup",
        title: "The test builds a counterparty the evidence does not cover",
        severity: Severity::Warn,
        severity_note: "warn; silenced for a name listed under deployment.evidence",
        summary: "The test deploys a stand-in (a strategy, a malicious target, a hook) or calls `ship`, `dock` or `push` on a protocol contract, and `deployment.evidence` in realism.yaml does not name that contract or that actor.",
        why: "A local `new Protocol()` is a normal harness and is not this rule. What this rule is for is the behavior the test invents on top of it: a maker strategy nobody's deployment ships, a dock, a push performed outside the path the protocol's own docs allow, a malicious target the test wrote. The profit then depends on a counterparty the test created. The tool cannot read those docs. `deployment.evidence` is where the author cites the deployment address, the config, and the page a reader can check. A missing citation is the finding.",
        fix: "Cite the deployment under `deployment.evidence` with the contract or actor name, the address, the config, and a source a reader can open. If there is no such deployment, the report has to say the behavior is one the test constructed.",
        bad: "vm.prank(maker);\naqua.ship(address(router), abi.encode(order), tokens, amounts);",
        good: "vm.prank(attacker);\nvault.deposit(1e18, attacker);",
        false_positives: &[
            "A contract the attacker deploys as their own (`MaliciousToken`, an exploit helper). The name matches a stand-in, and on the labelled 0.5.0 sample this was most of the flags. The attacker could have deployed it. An `allow` comment is the silence.",
            "A permissionless `ship` or `push` that any user can do, and that real users do. The rule cannot read the docs that say so. An evidence entry, or an `allow` comment citing the page, is the record.",
            "`array.push` on a memory array is not flagged; only a call on a contract the harness treats as the protocol. A helper the tool misreads as the protocol will be flagged. Helpers defined in the test's own file are not protocol contracts.",
            "The evidence entry is trusted, not checked. A citation that names the wrong deployment silences the rule and is visible in realism.yaml for the triager to read.",
        ],
        move_lang: MoveNotes::NotChecked(
            "Move tests are not checked for unevidenced setup. There is no `deployment.evidence` match against Move calls yet.",
        ),
        solana: SolanaNotes::NotChecked(
            "Solana tests are not checked for unevidenced setup. A privileged signer in the exploit is RG02.",
        ),
    },
    RuleInfo {
        id: "RG18",
        slug: "revert-only",
        title: "The only assertion is that a call reverts",
        severity: Severity::Warn,
        severity_note: "warn",
        summary: "Every assertion in the test is `vm.expectRevert` (or, in Move, the test is `#[expected_failure]` and contains no `assert!`). Nothing checks a balance, a profit, or a stuck value.",
        why: "A revert shows the call failed. It does not show that funds are locked, that a position is stranded, or that an attacker gained. A denial of service that locks value is a real finding only when the test shows the value still there and the exit still reverting. A revert that locks nothing is the finding that does not survive triage, and the test cannot tell the two apart.",
        fix: "Assert the impact next to the revert: the balance that stayed, the shares that cannot be redeemed, the difference against the baseline. If the impact is only the revert, say so and price it as a denial of service with no loss.",
        bad: "vm.prank(attacker);\nvm.expectRevert();\nvault.withdraw(1);",
        good: "uint256 before = token.balanceOf(address(vault));\nvm.prank(attacker);\nvm.expectRevert();\nvault.withdraw(1);\nassertEq(token.balanceOf(address(vault)), before);",
        false_positives: &[
            "A revert that is the whole impact (a permanent denial of service on a function nobody needs a balance to care about). The rule cannot know the function matters. An `allow` comment stating the impact is the way to keep it.",
            "An assertion of the locked balance in a helper the tool does not inline. Move the check into the test.",
        ],
        move_lang: MoveNotes::Checked {
            summary: "The test is `#[expected_failure]` and its body has no `assert!`. The pass condition is an abort.",
            fix: "Assert the value that stays stuck, then let the abort happen, or drop `expected_failure` and assert the state.",
            bad: "#[expected_failure]\n#[test]\nfun test_withdraw_aborts() {\n    let attacker = @0xA11CE;\n    test_scenario::next_tx(&mut scenario, attacker);\n    vault::withdraw(test_scenario::ctx(&mut scenario));\n}",
            good: "#[test]\nfun test_withdraw_leaves_the_balance() {\n    let attacker = @0xA11CE;\n    test_scenario::next_tx(&mut scenario, attacker);\n    let before = coin::value(&vault_coin);\n    vault::withdraw(test_scenario::ctx(&mut scenario));\n    assert!(coin::value(&vault_coin) == before, 0);\n}",
            false_positives: &[
                "An expected abort that is the impact. The rule cannot know that. An `allow` comment stating it is the record.",
            ],
        },
        solana: SolanaNotes::NotChecked(
            "A Rust `#[should_panic]` test is not checked. Solana tests that only unwrap a transaction already trip RG08 when they assert nothing else.",
        ),
    },
];

pub fn rule(id: &str) -> Option<&'static RuleInfo> {
    let id = id.trim();
    RULES
        .iter()
        .find(|r| r.id.eq_ignore_ascii_case(id) || r.slug.eq_ignore_ascii_case(id))
}

/// Renders a rule as the Markdown page published under `docs/rules/`.
pub fn markdown(r: &RuleInfo) -> String {
    let mut out = String::new();
    out.push_str(&format!("# {} `{}`: {}\n\n", r.id, r.slug, r.title));
    out.push_str(
        "<!-- Generated by `realism-gate explain --markdown`. Do not edit by hand. -->\n\n",
    );
    out.push_str(&format!(
        "**Severity:** {} ({})\n\n",
        r.severity.as_str(),
        r.severity_note
    ));
    out.push_str(&format!("{}\n\n", r.summary));
    out.push_str(&format!("## Why it matters\n\n{}\n\n", r.why));
    out.push_str(&format!("## How to fix it\n\n{}\n\n", r.fix));
    let lang = if r.id == "RG12" || r.id == "RG13" || r.id == "RG14" {
        "yaml"
    } else {
        "solidity"
    };
    if !r.bad.is_empty() {
        out.push_str(&format!("## Fires on\n\n```{lang}\n{}\n```\n\n", r.bad));
    }
    if !r.good.is_empty() {
        out.push_str(&format!(
            "## Does not fire on\n\n```{lang}\n{}\n```\n\n",
            r.good
        ));
    }
    out.push_str("## How it can be wrong\n\n");
    if r.false_positives.is_empty() {
        out.push_str("None known. This rule reads a declaration you wrote, not your code.\n\n");
    } else {
        for fp in r.false_positives {
            out.push_str(&format!("- {fp}\n"));
        }
        out.push('\n');
    }
    out.push_str("## In Solana tests (Rust and Anchor TypeScript)\n\n");
    match r.solana {
        SolanaNotes::Checked {
            summary,
            fix,
            rust_bad,
            rust_good,
            ts_bad,
            ts_good,
            false_positives,
        } => {
            out.push_str(&format!("{summary}\n\n**How to fix it:** {fix}\n\n"));
            for (what, rust, ts) in [
                ("Fires on", rust_bad, ts_bad),
                ("Does not fire on", rust_good, ts_good),
            ] {
                out.push_str(&format!(
                    "### {what}\n\n```rust\n{rust}\n```\n\n```typescript\n{ts}\n```\n\n"
                ));
            }
            out.push_str("### Known false-positive modes\n\n");
            for fp in false_positives {
                out.push_str(&format!("- {fp}\n"));
            }
            out.push('\n');
        }
        SolanaNotes::NotChecked(why) => out.push_str(&format!("Not checked. {why}\n\n")),
        SolanaNotes::Declaration => out.push_str(
            "The same: the rule reads `realism.yaml`, whatever language the tests are written in.\n\n",
        ),
    }
    out.push_str("## In Move tests (Sui and Aptos)\n\n");
    match r.move_lang {
        MoveNotes::Checked {
            summary,
            fix,
            bad,
            good,
            false_positives,
        } => {
            out.push_str(&format!("{summary}\n\n**How to fix it:** {fix}\n\n"));
            out.push_str(&format!(
                "### Fires on\n\n```move\n{bad}\n```\n\n### Does not fire on\n\n```move\n{good}\n```\n\n"
            ));
            out.push_str("### How it can be wrong\n\n");
            for fp in false_positives {
                out.push_str(&format!("- {fp}\n"));
            }
            out.push('\n');
        }
        MoveNotes::NotChecked(why) => out.push_str(&format!("Not checked. {why}\n\n")),
    }
    out.push_str(&format!(
        "## Silencing it\n\nAdd a comment on the line, or the line above, with a reason:\n\n```solidity\n// realism-gate: allow({}) <why this is not a defect here>\n```\n\nThe comment is the same in Rust, TypeScript and Move. A comment without a reason is ignored, and the output says so.\n",
        r.id
    ));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_and_slugs_are_unique_and_ordered() {
        for (i, r) in RULES.iter().enumerate() {
            assert_eq!(r.id, format!("RG{:02}", i + 1));
            assert!(RULES.iter().filter(|o| o.slug == r.slug).count() == 1);
        }
    }

    #[test]
    fn lookup_by_id_or_slug() {
        assert_eq!(rule("rg04").unwrap().slug, "storage-write");
        assert_eq!(rule("no-baseline").unwrap().id, "RG10");
        assert!(rule("RG99").is_none());
    }

    #[test]
    fn every_code_rule_documents_a_false_positive_mode() {
        for r in RULES {
            if matches!(r.solana, SolanaNotes::Declaration) {
                continue;
            }
            assert!(!r.false_positives.is_empty(), "{} lists none", r.id);
            match r.solana {
                SolanaNotes::Checked {
                    false_positives, ..
                } => assert!(
                    !false_positives.is_empty(),
                    "{} lists none for Solana",
                    r.id
                ),
                SolanaNotes::NotChecked(why) => assert!(!why.is_empty()),
                SolanaNotes::Declaration => unreachable!(),
            }
            match r.move_lang {
                MoveNotes::Checked {
                    false_positives, ..
                } => assert!(!false_positives.is_empty(), "{} lists none for Move", r.id),
                MoveNotes::NotChecked(why) => assert!(!why.is_empty(), "{}", r.id),
            }
        }
    }

    #[test]
    fn only_the_declaration_rules_are_language_neutral() {
        for r in RULES {
            let neutral = matches!(r.solana, SolanaNotes::Declaration);
            assert_eq!(
                neutral,
                ["RG12", "RG13", "RG14"].contains(&r.id),
                "{}",
                r.id
            );
        }
    }
}
