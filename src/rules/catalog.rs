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
}

/// The source language a finding was raised in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lang {
    Solidity,
    Rust,
    TypeScript,
}

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
        match (lang, self.solana) {
            (Lang::Rust | Lang::TypeScript, SolanaNotes::Checked { fix, .. }) => fix,
            _ => self.fix,
        }
    }

    /// Whether the rule applies to Solana tests, directly or through the
    /// declaration.
    pub fn checks_solana(&self) -> bool {
        !matches!(self.solana, SolanaNotes::NotChecked(_))
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
        severity_note: "warn",
        summary: "The test contract deploys the code under review in setUp() and then calls it without changing sender, so every call runs as the deployer.",
        why: "Constructors commonly give the deployer ownership or every role. A test that deploys with `new Vault()` and then calls `vault.sweep()` from the test contract has proved that the deployer can sweep. Whether an outsider can is untested. This is the single most common way an honest PoC accidentally proves a privileged action.",
        fix: "Deploy and configure as one address, attack as another: `vm.startPrank(attacker)` before the exploit steps, or transfer ownership to a dedicated owner in `setUp()`.",
        bad: "function setUp() public { vault = new Vault(); }\nfunction test_sweep() public {\n    vault.sweep(address(token)); // runs as the deployer\n}",
        good: "function setUp() public { vault = new Vault(); }\nfunction test_sweep() public {\n    vm.prank(attacker);\n    vault.sweep(address(token));\n}",
        false_positives: &[
            "A contract with no access control at all, where the deployer has no extra power. The finding is still worth an `allow` comment saying so, because the triager will ask.",
            "A constructor that hands ownership to an argument that the tool does not read as privileged. Pass a variable named for the role (`owner`, `admin`) or label it.",
        ],
        solana: SolanaNotes::Checked {
            summary: "The exploit's transactions are signed only by the keys that signed the setup, usually the harness's `payer` or Anchor's `provider.wallet` (which signs every `.rpc()`), and by no separate attacker keypair.",
            fix: "Set up as one keypair, attack as another: create an `attacker` keypair, fund it, and have it sign the exploit.",
            rust_bad: "let (mut banks, payer, _) = program_test.start().await;\nprocess(&mut banks, &[init_pool_ix(&payer.pubkey())], &[&payer]).await;\nprocess(&mut banks, &[sweep_ix(&payer.pubkey())], &[&payer]).await;",
            rust_good: "process(&mut banks, &[init_pool_ix(&payer.pubkey())], &[&payer]).await;\nlet attacker = Keypair::new();\nprocess(&mut banks, &[sweep_ix(&attacker.pubkey())], &[&payer, &attacker]).await;",
            ts_bad: "await program.methods.initPool().accounts({ payer: provider.wallet.publicKey }).rpc();\nawait program.methods.sweep().accounts({ recipient: provider.wallet.publicKey }).rpc();",
            ts_good: "await program.methods.initPool().accounts({ payer: provider.wallet.publicKey }).rpc();\nawait program.methods.sweep().accounts({ recipient: attacker.publicKey }).signers([attacker]).rpc();",
            false_positives: &[
                "A program with no access control, where the setup key gains nothing. Add an `allow` comment saying so; the triager will ask.",
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
        solana: SolanaNotes::Declaration,
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
    out.push_str("## Known false-positive modes\n\n");
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
    out.push_str(&format!(
        "## Silencing it\n\nAdd a comment on the line, or the line above, with a reason:\n\n```solidity\n// realism-gate: allow({}) <why this is not a defect here>\n```\n\nThe comment is the same in Rust and TypeScript. A comment without a reason is ignored, and the output says so.\n",
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
        for r in RULES.iter().take(11) {
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
                SolanaNotes::Declaration => panic!("{} reads code, not the declaration", r.id),
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
