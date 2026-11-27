//! F1: RG02/RG03 fire only on an authority holder the test binds to an actor
//! that is not the attacker.
//!
//! The DEV error classes (P2R-2 §2 F1; two blind readers, kappa 0.72) are
//! RG02 over-firing on participant keys that the program model says sign an
//! entry point but that the test never ties to anyone but the attacker:
//!
//! - **generated participant**: a key the test generates and funds, with no
//!   protocol role, signs its own stake/claim/contribution (4 instances);
//! - **generated creator of its own resource**: a generated key creates a
//!   resource through a permissionless entry point and then acts on it
//!   (1 instance);
//! - **unbound fragment participant**: an extracted fragment uses a key whose
//!   origin it does not show to contribute or reclaim (3 instances);
//! - **unbound fragment creator**: the same, for the key that created the
//!   resource and then reconfigures or drains it (4 instances).
//!
//! Each of the 12 instances has a must-not-fire fixture below, and each class
//! has a must-still-fire twin that differs only by a binding. The fixtures
//! are synthetic: the programs (`pledgebox`, `yieldfarm`, `questboard`) and the
//! tests were written for this file, not copied from the corpus.
//!
//! Every fixture runs through the binary with `--model-target`, as the DEV
//! run did.

use proptest::prelude::*;
use realism_gate::engine::{Options, analyze};
use realism_gate::project::Project;
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

// ---------------------------------------------------------------------------
// The synthetic programs (the `--model-target` checkout)

const PROGRAMS: &str = r#"
use anchor_lang::prelude::*;
declare_id!("P1edgeBox11111111111111111111111111111111111");

#[program]
pub mod pledgebox {
    use super::*;
    pub fn open_campaign(ctx: Context<OpenCampaign>, goal: u64) -> Result<()> { Ok(()) }
    pub fn pledge(ctx: Context<Pledge>, amount: u64) -> Result<()> { Ok(()) }
    pub fn reclaim(ctx: Context<Reclaim>) -> Result<()> { Ok(()) }
    pub fn set_closing_time(ctx: Context<SetClosingTime>, at: i64) -> Result<()> { Ok(()) }
    pub fn collect(ctx: Context<Collect>) -> Result<()> { Ok(()) }
    pub fn init_registry(ctx: Context<InitRegistry>) -> Result<()> { Ok(()) }
}

#[derive(Accounts)]
pub struct OpenCampaign<'info> {
    #[account(mut)]
    pub campaign: AccountInfo<'info>,
    #[account(mut)]
    pub organizer: Signer<'info>,
}
#[derive(Accounts)]
pub struct Pledge<'info> {
    #[account(mut)]
    pub campaign: AccountInfo<'info>,
    #[account(mut)]
    pub backer: Signer<'info>,
}
#[derive(Accounts)]
pub struct Reclaim<'info> {
    #[account(mut)]
    pub campaign: AccountInfo<'info>,
    #[account(mut)]
    pub backer: Signer<'info>,
}
#[derive(Accounts)]
pub struct SetClosingTime<'info> {
    #[account(mut)]
    pub campaign: AccountInfo<'info>,
    pub organizer: Signer<'info>,
}
#[derive(Accounts)]
pub struct Collect<'info> {
    #[account(mut)]
    pub campaign: AccountInfo<'info>,
    #[account(mut)]
    pub organizer: Signer<'info>,
}
#[derive(Accounts)]
pub struct InitRegistry<'info> {
    #[account(mut)]
    pub registry: AccountInfo<'info>,
    #[account(mut)]
    pub admin: Signer<'info>,
    pub organizer: AccountInfo<'info>,
    pub backer: AccountInfo<'info>,
}

#[program]
pub mod yieldfarm {
    use super::*;
    pub fn initialize(ctx: Context<Initialize>, rate: u64) -> Result<()> { Ok(()) }
    pub fn lock_stake(ctx: Context<LockStake>, amount: u64) -> Result<()> { Ok(()) }
    pub fn claim_yield(ctx: Context<ClaimYield>) -> Result<()> { Ok(()) }
    pub fn update_rates(ctx: Context<UpdateRates>, rate: u64) -> Result<()> { Ok(()) }
    pub fn set_staker_tier(ctx: Context<SetStakerTier>, tier: u8) -> Result<()> { Ok(()) }
}

#[derive(Accounts)]
pub struct Initialize<'info> {
    #[account(mut)]
    pub farm: AccountInfo<'info>,
    #[account(mut)]
    pub admin: Signer<'info>,
}
#[derive(Accounts)]
pub struct LockStake<'info> {
    #[account(mut)]
    pub farm: AccountInfo<'info>,
    #[account(mut)]
    pub staker: Signer<'info>,
}
#[derive(Accounts)]
pub struct ClaimYield<'info> {
    #[account(mut)]
    pub farm: AccountInfo<'info>,
    #[account(mut)]
    pub staker: Signer<'info>,
}
#[derive(Accounts)]
pub struct UpdateRates<'info> {
    #[account(mut)]
    pub farm: AccountInfo<'info>,
    pub authority: Signer<'info>,
}
#[derive(Accounts)]
pub struct SetStakerTier<'info> {
    #[account(mut)]
    pub farm: AccountInfo<'info>,
    pub authority: Signer<'info>,
    pub staker: AccountInfo<'info>,
}

#[program]
pub mod questboard {
    use super::*;
    pub fn setup(ctx: Context<Setup>) -> Result<()> { Ok(()) }
    pub fn add_moderator(ctx: Context<AddModerator>) -> Result<()> { Ok(()) }
    pub fn post_quest(ctx: Context<PostQuest>, reward: u64) -> Result<()> { Ok(()) }
    pub fn review_quest(ctx: Context<ReviewQuest>) -> Result<()> { Ok(()) }
    pub fn suspend_quest(ctx: Context<SuspendQuest>) -> Result<()> { Ok(()) }
    pub fn mark_failed(ctx: Context<MarkFailed>) -> Result<()> { Ok(()) }
    pub fn abandon_quest(ctx: Context<AbandonQuest>) -> Result<()> { Ok(()) }
    pub fn change_options(ctx: Context<ChangeOptions>, fee: u64) -> Result<()> { Ok(()) }
    pub fn set_host_badge(ctx: Context<SetHostBadge>) -> Result<()> { Ok(()) }
}

#[derive(Accounts)]
pub struct Setup<'info> {
    #[account(mut)]
    pub board: AccountInfo<'info>,
    #[account(mut)]
    pub owner: Signer<'info>,
}
#[derive(Accounts)]
pub struct AddModerator<'info> {
    #[account(mut)]
    pub board: AccountInfo<'info>,
    #[account(mut)]
    pub owner: Signer<'info>,
    pub moderator: AccountInfo<'info>,
}
#[derive(Accounts)]
pub struct PostQuest<'info> {
    #[account(mut)]
    pub quest: AccountInfo<'info>,
    #[account(mut)]
    pub host: Signer<'info>,
}
#[derive(Accounts)]
pub struct ReviewQuest<'info> {
    #[account(mut)]
    pub quest: AccountInfo<'info>,
    pub moderator: Signer<'info>,
}
#[derive(Accounts)]
pub struct SuspendQuest<'info> {
    #[account(mut)]
    pub quest: AccountInfo<'info>,
    pub moderator: Signer<'info>,
}
#[derive(Accounts)]
pub struct MarkFailed<'info> {
    #[account(mut)]
    pub quest: AccountInfo<'info>,
    pub moderator: Signer<'info>,
}
#[derive(Accounts)]
pub struct AbandonQuest<'info> {
    #[account(mut)]
    pub quest: AccountInfo<'info>,
    #[account(mut)]
    pub host: Signer<'info>,
}
#[derive(Accounts)]
pub struct ChangeOptions<'info> {
    #[account(mut)]
    pub board: AccountInfo<'info>,
    pub owner: Signer<'info>,
}
#[derive(Accounts)]
pub struct SetHostBadge<'info> {
    #[account(mut)]
    pub board: AccountInfo<'info>,
    pub owner: Signer<'info>,
    pub host: AccountInfo<'info>,
}
"#;

/// A fresh scratch directory under the system temp dir, unique per test.
fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("rg-f1-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// Writes the synthetic programs as an Anchor checkout under `dir/target`.
fn write_target(dir: &Path) -> PathBuf {
    let target = dir.join("target-checkout");
    std::fs::create_dir_all(target.join("programs/suite/src")).unwrap();
    std::fs::write(target.join("programs/suite/src/lib.rs"), PROGRAMS).unwrap();
    target
}

/// The program model the library-level tests share.
fn model() -> &'static rg_program::ProgramModel {
    static MODEL: OnceLock<rg_program::ProgramModel> = OnceLock::new();
    MODEL.get_or_init(|| {
        let dir = scratch("shared-model");
        let target = write_target(&dir);
        rg_program::ProgramModel::resolve(&target).expect("the synthetic programs resolve")
    })
}

/// One finding: rule and line.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct Hit {
    rule: String,
    line: u64,
}

/// Runs the binary on one test file, with `--model-target`, and returns the
/// active findings and the raw stdout.
fn run_cli(name: &str, file: &str, source: &str, fragment: bool) -> (Vec<Hit>, Vec<u8>) {
    let dir = scratch(name);
    let target = write_target(&dir);
    std::fs::write(dir.join(file), source).unwrap();
    let mut args = vec![
        "check".to_string(),
        file.to_string(),
        "--no-declaration".to_string(),
        "--no-config".to_string(),
        "--format".to_string(),
        "json".to_string(),
        "--fail-on".to_string(),
        "never".to_string(),
        "--model-target".to_string(),
        target.to_string_lossy().to_string(),
    ];
    if fragment {
        args.push("--fragment".to_string());
    }
    let out = std::process::Command::new(env!("CARGO_BIN_EXE_realism-gate"))
        .args(&args)
        .current_dir(&dir)
        .env("NO_COLOR", "1")
        .output()
        .expect("the binary runs");
    assert!(
        out.status.success(),
        "{name}: exit {:?}\n{}",
        out.status.code(),
        String::from_utf8_lossy(&out.stderr)
    );
    let report: Value = serde_json::from_slice(&out.stdout).expect("JSON report");
    assert_eq!(
        report["parse_errors"].as_array().map_or(0, Vec::len),
        0,
        "{name}: {}",
        report["parse_errors"]
    );
    assert!(
        report["summary"]["tests"].as_u64().unwrap_or(0) > 0,
        "{name}: no test was read"
    );
    let mut hits: Vec<Hit> = report["findings"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|f| f.get("suppressed").is_none())
        .map(|f| Hit {
            rule: f["rule"].as_str().unwrap().to_string(),
            line: f["line"].as_u64().unwrap(),
        })
        .collect();
    hits.sort();
    let _ = std::fs::remove_dir_all(&dir);
    (hits, out.stdout)
}

fn hits(name: &str, source: &str) -> Vec<Hit> {
    run_cli(name, "poc.ts", source, false).0
}

fn fragment_hits(name: &str, source: &str) -> Vec<Hit> {
    run_cli(name, "poc.ts", source, true).0
}

/// The 1-based line of the first line of `source` that contains `needle`
/// after the line containing `after`.
fn line_of(source: &str, after: &str, needle: &str) -> u64 {
    let start = source
        .lines()
        .position(|l| l.contains(after))
        .unwrap_or_else(|| panic!("no line with {after:?}"));
    let i = source
        .lines()
        .enumerate()
        .skip(start)
        .find(|(_, l)| l.contains(needle))
        .unwrap_or_else(|| panic!("no line with {needle:?} after {after:?}"))
        .0;
    i as u64 + 1
}

fn fired(hits: &[Hit], rule: &str) -> Vec<u64> {
    hits.iter()
        .filter(|h| h.rule == rule)
        .map(|h| h.line)
        .collect()
}

const TS_HEAD: &str = r#"import * as anchor from "@coral-xyz/anchor";
import { BN, Program } from "@coral-xyz/anchor";
import { Keypair, LAMPORTS_PER_SOL, PublicKey, Transaction, SystemProgram } from "@solana/web3.js";
import { assert } from "chai";
"#;

// ---------------------------------------------------------------------------
// Class: generated participant with no protocol role (4 DEV instances)

/// Instances 1 and 2: a generated, airdropped staker locks and claims. The
/// program says both entry points need the staker's signature; nothing makes
/// the staker anyone but the attacker's own key.
fn generated_staker(binding: &str) -> String {
    format!(
        r#"{TS_HEAD}
describe("yield over-accrual", () => {{
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);
  const program = anchor.workspace.Yieldfarm as Program<any>;
  const admin = (provider.wallet as anchor.Wallet).payer;

  it("claims yield at the new rate for the whole period", async () => {{
    const [farm] = PublicKey.findProgramAddressSync([Buffer.from("farm")], program.programId);
    await program.methods.initialize(new BN(10)).accounts({{ farm, admin: admin.publicKey }}).rpc();
    const staker = Keypair.generate();
    const sig = await provider.connection.requestAirdrop(staker.publicKey, 3 * LAMPORTS_PER_SOL);
    await provider.connection.confirmTransaction(sig);
{binding}
    await program.methods
      .lockStake(new BN(500))
      .accounts({{ farm, staker: staker.publicKey }})
      .signers([staker])
      .rpc();
    const before = await provider.connection.getBalance(staker.publicKey);
    await program.methods
      .claimYield()
      .accounts({{ farm, staker: staker.publicKey }})
      .signers([staker])
      .rpc();
    const after = await provider.connection.getBalance(staker.publicKey);
    assert.isAbove(after, before);
  }});
}});
"#
    )
}

#[test]
fn f1_generated_participant_staker_does_not_fire() {
    let src = generated_staker("");
    let h = hits("gen-staker", &src);
    let lock = line_of(&src, "lockStake", "lockStake");
    let claim = line_of(&src, "claimYield", "claimYield");
    let rg02 = fired(&h, "RG02");
    // DEV instances 1 (the stake) and 2 (the claim).
    assert!(
        !rg02.contains(&(lock - 1)) && !rg02.contains(&lock),
        "{h:?}"
    );
    assert!(
        !rg02.contains(&(claim - 1)) && !rg02.contains(&claim),
        "{h:?}"
    );
    assert!(rg02.is_empty(), "{h:?}");
}

#[test]
fn f1_generated_participant_staker_still_fires_when_a_protocol_role_binds_it() {
    // The twin: the admin, a default signer that initialised the farm, gives
    // the staker a tier before the exploit. That is a protocol role granted by
    // an actor that is not the attacker, so the staker's signature is one the
    // attacker cannot produce.
    let src = generated_staker(
        "    await program.methods.setStakerTier(2).accounts({ farm, authority: admin.publicKey, staker: staker.publicKey }).rpc();",
    );
    let h = hits("gen-staker-twin", &src);
    assert!(!fired(&h, "RG02").is_empty(), "{h:?}");
}

/// Instances 3 and 4: the organizer is the provider wallet with no protocol
/// role, and a generated, airdropped backer pledges in two tests.
fn wallet_organizer_backer(binding: &str) -> String {
    format!(
        r#"{TS_HEAD}
describe("campaign closes early", () => {{
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);
  const program = anchor.workspace.Pledgebox as Program<any>;
  const organizer = provider.wallet;
  const backer = Keypair.generate();
  let campaign: PublicKey;

  before(async () => {{
    [campaign] = PublicKey.findProgramAddressSync([Buffer.from("c1"), organizer.publicKey.toBuffer()], program.programId);
    const sig = await provider.connection.requestAirdrop(backer.publicKey, 4 * LAMPORTS_PER_SOL);
    await provider.connection.confirmTransaction(sig);
{binding}
  }});

  it("pledges half the goal", async () => {{
    await program.methods.openCampaign(new BN(2_000)).accounts({{ campaign, organizer: organizer.publicKey }}).rpc();
    const before = await provider.connection.getBalance(campaign);
    await program.methods
      .pledge(new BN(1_000))
      .accounts({{ campaign, backer: backer.publicKey }})
      .signers([backer])
      .rpc();
    const after = await provider.connection.getBalance(campaign);
    assert.isAbove(after, before);
  }});

  it("organizer collects before the goal", async () => {{
    const before = await provider.connection.getBalance(organizer.publicKey);
    await program.methods
      .pledge(new BN(1_000))
      .accounts({{ campaign, backer: backer.publicKey }})
      .signers([backer])
      .rpc();
    await program.methods.collect().accounts({{ campaign, organizer: organizer.publicKey }}).rpc();
    const after = await provider.connection.getBalance(organizer.publicKey);
    assert.isAbove(after, before);
  }});
}});
"#
    )
}

#[test]
fn f1_generated_participant_backer_does_not_fire() {
    let src = wallet_organizer_backer("");
    let h = hits("gen-backer", &src);
    // DEV instances 3 and 4 (the two pledges); the organizer is the wallet
    // with no protocol role, so RG03 on its collect is the same class.
    assert!(fired(&h, "RG02").is_empty(), "{h:?}");
    assert!(fired(&h, "RG03").is_empty(), "{h:?}");
}

#[test]
fn f1_generated_participant_backer_still_fires_when_a_protocol_role_binds_it() {
    // The twin: a separate registry admin registers the backer before the
    // exploit, so the backer holds a role the attacker did not hand itself.
    let src = wallet_organizer_backer(
        "    const admin = anchor.web3.Keypair.fromSecretKey(Uint8Array.from(REGISTRY_ADMIN_SECRET));\n    await program.methods.initRegistry().accounts({ registry: campaign, admin: admin.publicKey, backer: backer.publicKey }).signers([admin]).rpc();",
    );
    let h = hits("gen-backer-twin", &src);
    assert!(!fired(&h, "RG02").is_empty(), "{h:?}");
}

// ---------------------------------------------------------------------------
// Class: generated creator acting on its own resource (1 DEV instance)

fn generated_host(binding: &str) -> String {
    format!(
        r#"{TS_HEAD}
describe("abandon refund uses the current fee", () => {{
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);
  const program = anchor.workspace.Questboard as Program<any>;
  const owner = provider.wallet as anchor.Wallet;
  const host = Keypair.generate();
  let board: PublicKey;

  before(async () => {{
    const sig = await provider.connection.requestAirdrop(host.publicKey, 20 * LAMPORTS_PER_SOL);
    await provider.connection.confirmTransaction(sig);
    [board] = PublicKey.findProgramAddressSync([Buffer.from("board")], program.programId);
    await program.methods.setup().accounts({{ board, owner: owner.publicKey }}).rpc();
  }});

  it("host loses the refund", async () => {{
    const quest = Keypair.generate();
    await program.methods
      .postQuest(new BN(10))
      .accounts({{ quest: quest.publicKey, host: host.publicKey }})
      .signers([host, quest])
      .rpc();
{binding}
    const before = await provider.connection.getBalance(host.publicKey);
    await program.methods.changeOptions(new BN(9)).accounts({{ board, owner: owner.publicKey }}).rpc();
    await program.methods
      .abandonQuest()
      .accounts({{ quest: quest.publicKey, host: host.publicKey }})
      .signers([host])
      .rpc();
    const after = await provider.connection.getBalance(host.publicKey);
    assert.isBelow(after - before, 1 * LAMPORTS_PER_SOL);
  }});
}});
"#
    )
}

#[test]
fn f1_generated_creator_of_own_resource_does_not_fire() {
    let src = generated_host("");
    let h = hits("gen-host", &src);
    let abandon = line_of(&src, "abandonQuest", "abandonQuest");
    let rg02 = fired(&h, "RG02");
    // DEV instance 5: the host abandoning the quest it posted.
    assert!(
        !rg02.contains(&(abandon - 1)) && !rg02.contains(&abandon),
        "{h:?}"
    );
    assert!(rg02.is_empty(), "{h:?}");
}

#[test]
fn f1_generated_creator_of_own_resource_still_fires_when_the_owner_badges_it() {
    // The twin: the board owner, a default signer bound to the `owner` role by
    // setup, gives the host a badge before the exploit.
    let src = generated_host(
        "    await program.methods.setHostBadge().accounts({ board, owner: owner.publicKey, host: host.publicKey }).rpc();",
    );
    let h = hits("gen-host-twin", &src);
    assert!(!fired(&h, "RG02").is_empty(), "{h:?}");
}

// ---------------------------------------------------------------------------
// Class: unbound fragment participant (3 DEV instances) and unbound fragment
// creator (4 DEV instances). A fragment pasted from a report omits where its
// keys come from.

/// Instances 6 (pledge), 7 (set_closing_time) and 8 (reclaim).
fn fragment_pledge_reclaim(binding: &str) -> String {
    format!(
        r#"{binding}// Open the campaign with a goal of 5
await program.methods
  .openCampaign(new anchor.BN(5_000))
  .accounts({{ campaign, organizer: organizer.publicKey }})
  .signers([organizer])
  .rpc();

// Pledge 2
await program.methods
  .pledge(new anchor.BN(2_000))
  .accounts({{ campaign, backer: backer.publicKey }})
  .signers([backer])
  .rpc();

// Move the closing time into the past
await program.methods
  .setClosingTime(new anchor.BN(1))
  .accounts({{ campaign, organizer: organizer.publicKey }})
  .signers([organizer])
  .rpc();

const before = await provider.connection.getBalance(backer.publicKey);
await program.methods
  .reclaim()
  .accounts({{ campaign, backer: backer.publicKey }})
  .signers([backer])
  .rpc();
const after = await provider.connection.getBalance(backer.publicKey);
console.log("reclaimed", after - before);
"#
    )
}

#[test]
fn f1_unbound_fragment_participant_and_creator_do_not_fire() {
    let src = fragment_pledge_reclaim("");
    let h = fragment_hits("frag-reclaim", &src);
    assert!(fired(&h, "RG02").is_empty(), "{h:?}");
    // The same fragment's RG03 on the organizer is the same class.
    assert!(fired(&h, "RG03").is_empty(), "{h:?}");
}

#[test]
fn f1_unbound_fragment_participant_still_fires_when_the_fragment_binds_it() {
    // The twin: the fragment shows that the backer is a key the test loads,
    // and a registry admin it also loads registers that backer: the backer is
    // bound to an actor that is not the attacker.
    let src = fragment_pledge_reclaim(
        "const admin = Keypair.fromSecretKey(Uint8Array.from(ADMIN_SECRET));\nconst backer = Keypair.fromSecretKey(Uint8Array.from(BACKER_SECRET));\nawait program.methods.initRegistry().accounts({ registry: campaign, admin: admin.publicKey, backer: backer.publicKey }).signers([admin]).rpc();\n",
    );
    let h = fragment_hits("frag-reclaim-twin", &src);
    assert!(!fired(&h, "RG02").is_empty(), "{h:?}");
}

/// Instances 9 (pledge) and 10 (collect).
fn fragment_collect(binding: &str) -> String {
    format!(
        r#"{binding}await program.methods.openCampaign(new anchor.BN(5_000))
  .accounts({{ campaign, organizer: organizer.publicKey }})
  .signers([organizer])
  .rpc();

await program.methods.pledge(new anchor.BN(5_000))
  .accounts({{ campaign, backer: backer.publicKey }})
  .signers([backer])
  .rpc();

const stateBefore = await program.account.campaign.fetch(campaign);
await program.methods.collect()
  .accounts({{ campaign, organizer: organizer.publicKey }})
  .signers([organizer])
  .rpc();
const stateAfter = await program.account.campaign.fetch(campaign);
console.log(stateBefore.raised, stateAfter.raised);
"#
    )
}

#[test]
fn f1_unbound_fragment_creator_collect_does_not_fire() {
    let src = fragment_collect("");
    let h = fragment_hits("frag-collect", &src);
    assert!(fired(&h, "RG02").is_empty(), "{h:?}");
    assert!(fired(&h, "RG03").is_empty(), "{h:?}");
}

#[test]
fn f1_unbound_fragment_creator_still_fires_when_the_fragment_binds_it() {
    // The twin: the organizer is the provider wallet, and the fragment shows
    // a registry admin binding it before the exploit.
    let src = fragment_collect(
        "const admin = Keypair.fromSecretKey(Uint8Array.from(ADMIN_SECRET));\nconst organizer = Keypair.fromSecretKey(Uint8Array.from(ORGANIZER_SECRET));\nawait program.methods.initRegistry().accounts({ registry: campaign, admin: admin.publicKey, organizer: organizer.publicKey }).signers([admin]).rpc();\n",
    );
    let h = fragment_hits("frag-collect-twin", &src);
    assert!(!fired(&h, "RG02").is_empty(), "{h:?}");
}

/// Instances 11 and 12: the organizer sets the closing time twice.
fn fragment_double_set(binding: &str) -> String {
    format!(
        r#"{binding}
await program.methods
  .openCampaign(new anchor.BN(5_000))
  .accounts({{ campaign, organizer: organizer.publicKey }})
  .signers([organizer])
  .rpc();

const flagBefore = await program.account.campaign.fetch(campaign);
await program.methods
  .setClosingTime(new anchor.BN(86_400))
  .accounts({{ campaign, organizer: organizer.publicKey }})
  .signers([organizer])
  .rpc();

await program.methods
  .setClosingTime(new anchor.BN(172_800))
  .accounts({{ campaign, organizer: organizer.publicKey }})
  .signers([organizer])
  .rpc();
const flagAfter = await program.account.campaign.fetch(campaign);
console.log(flagBefore.closingSet, flagAfter.closingSet);
"#
    )
}

#[test]
fn f1_unbound_fragment_creator_double_set_does_not_fire() {
    let src = fragment_double_set("");
    let h = fragment_hits("frag-double-set", &src);
    assert!(fired(&h, "RG02").is_empty(), "{h:?}");
    assert!(fired(&h, "RG03").is_empty(), "{h:?}");
}

#[test]
fn f1_unbound_fragment_creator_double_set_still_fires_when_bound() {
    let src = fragment_double_set(
        "const admin = Keypair.fromSecretKey(Uint8Array.from(ADMIN_SECRET));\nconst organizer = Keypair.fromSecretKey(Uint8Array.from(ORGANIZER_SECRET));\nawait program.methods.initRegistry().accounts({ registry: campaign, admin: admin.publicKey, organizer: organizer.publicKey }).signers([admin]).rpc();",
    );
    let h = fragment_hits("frag-double-set-twin", &src);
    assert!(!fired(&h, "RG02").is_empty(), "{h:?}");
}

#[test]
fn f1_default_signer_without_a_protocol_role_is_the_attacker() {
    // The fragment says where the organizer comes from: the provider wallet,
    // which the test never gives a protocol role. The wallet is the attacker.
    let src = fragment_double_set("const organizer = provider.wallet;\n");
    let h = fragment_hits("frag-wallet-organizer", &src);
    assert!(fired(&h, "RG02").is_empty(), "{h:?}");
    assert!(fired(&h, "RG03").is_empty(), "{h:?}");
}

// ---------------------------------------------------------------------------
// True positives that must still fire: the two DEV RG02 TPs (a moderator a
// separate owner registered) and the two Arena RG03 TPs (the setup wallet
// that took the owner/admin role and then runs the gated exploit itself).

fn registered_moderator() -> String {
    format!(
        r#"{TS_HEAD}
describe("a ban reopens a closed market", () => {{
  let provider: any;
  let program: Program<any>;
  let owner: Keypair;
  let moderator: Keypair;
  let host: Keypair;
  let board: PublicKey;
  let quest: Keypair;

  before(async () => {{
    const context = await startAnchor(".", [], []);
    owner = context.payer;
    provider = new BankrunProvider(context);
    anchor.setProvider(provider);
    program = new Program<any>(IDL, provider);
    moderator = Keypair.generate();
    host = Keypair.generate();
    for (const kp of [moderator, host]) {{
      const tx = new Transaction().add(
        SystemProgram.transfer({{ fromPubkey: owner.publicKey, toPubkey: kp.publicKey, lamports: 5 * LAMPORTS_PER_SOL }})
      );
      await provider.sendAndConfirm(tx, [owner]);
    }}
    [board] = PublicKey.findProgramAddressSync([Buffer.from("board")], program.programId);
    await program.methods.setup().accounts({{ board, owner: owner.publicKey }}).rpc();
    await program.methods.addModerator().accounts({{ board, owner: owner.publicKey, moderator: moderator.publicKey }}).rpc();
    quest = Keypair.generate();
    await program.methods.postQuest(new BN(1)).accounts({{ quest: quest.publicKey, host: host.publicKey }}).signers([host, quest]).rpc();
    await program.methods.reviewQuest().accounts({{ quest: quest.publicKey, moderator: moderator.publicKey }}).signers([moderator]).rpc();
  }});

  it("suspends then fails the quest and trading reopens", async () => {{
    let state = await program.account.quest.fetch(quest.publicKey);
    assert.deepEqual(state.status, {{ open: {{}} }});
    await program.methods
      .suspendQuest()
      .accounts({{ quest: quest.publicKey, moderator: moderator.publicKey }})
      .signers([moderator])
      .rpc();
    await program.methods
      .markFailed()
      .accounts({{ quest: quest.publicKey, moderator: moderator.publicKey }})
      .signers([moderator])
      .rpc();
    state = await program.account.quest.fetch(quest.publicKey);
    assert.deepEqual(state.status, {{ failed: {{}} }});
  }});
}});
"#
    )
}

#[test]
fn f1_tp_registered_moderator_still_fires_rg02_on_both_steps() {
    let src = registered_moderator();
    let h = hits("tp-moderator", &src);
    // The flag sits on the `await program.methods` line that starts the
    // builder chain, just above the instruction line.
    let rg02 = fired(&h, "RG02");
    let near = |line: u64| rg02.iter().any(|&l| l + 2 >= line && l <= line + 1);
    let suspend_ix = line_of(&src, "it(\"suspends", ".suspendQuest()");
    let mark_ix = line_of(&src, "it(\"suspends", ".markFailed()");
    assert!(near(suspend_ix), "{h:?}");
    assert!(near(mark_ix), "{h:?}");
    // The moderator also built the world (it reviewed the quest in setup).
    assert!(!fired(&h, "RG03").is_empty(), "{h:?}");
}

fn self_registered_wallet_moderator() -> String {
    format!(
        r#"{TS_HEAD}
describe("owner retroactively raises the fee", () => {{
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);
  const program = anchor.workspace.Questboard as Program<any>;
  const admin = provider.wallet as anchor.Wallet;
  const host = Keypair.generate();
  let board: PublicKey;

  before(async () => {{
    const sig = await provider.connection.requestAirdrop(host.publicKey, 20 * LAMPORTS_PER_SOL);
    await provider.connection.confirmTransaction(sig);
    [board] = PublicKey.findProgramAddressSync([Buffer.from("board")], program.programId);
    await program.methods.setup().accounts({{ board, owner: admin.publicKey }}).rpc();
    await program.methods.addModerator().accounts({{ board, owner: admin.publicKey, moderator: admin.publicKey }}).rpc();
  }});

  it("host loses the refund", async () => {{
    const quest = Keypair.generate();
    await program.methods.postQuest(new BN(10)).accounts({{ quest: quest.publicKey, host: host.publicKey }}).signers([host, quest]).rpc();
    await program.methods.reviewQuest().accounts({{ quest: quest.publicKey, moderator: admin.publicKey }}).rpc();
    const before = await provider.connection.getBalance(host.publicKey);
    await program.methods.changeOptions(new BN(9)).accounts({{ board, owner: admin.publicKey }}).rpc();
    const after = await provider.connection.getBalance(host.publicKey);
    assert.isBelow(after - before, 1 * LAMPORTS_PER_SOL);
  }});
}});
"#
    )
}

#[test]
fn f1_tp_self_registered_wallet_owner_still_fires_rg03() {
    let src = self_registered_wallet_moderator();
    let h = hits("tp-wallet-moderator", &src);
    assert!(!fired(&h, "RG03").is_empty(), "{h:?}");
}

fn wallet_admin_updates_rates() -> String {
    format!(
        r#"{TS_HEAD}
describe("retroactive rate", () => {{
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);
  const program = anchor.workspace.Yieldfarm as Program<any>;
  const admin = (provider.wallet as anchor.Wallet).payer;

  it("applies the new rate to the whole period", async () => {{
    const [farm] = PublicKey.findProgramAddressSync([Buffer.from("farm")], program.programId);
    // The accounts name the system program: the call is still the farm's
    // own `initialize`, and it still makes the wallet the farm's admin.
    await program.methods
      .initialize(new BN(10))
      .accounts({{ farm, admin: admin.publicKey, systemProgram: anchor.web3.SystemProgram.programId }})
      .rpc();
    const staker = Keypair.generate();
    const sig = await provider.connection.requestAirdrop(staker.publicKey, 3 * LAMPORTS_PER_SOL);
    await provider.connection.confirmTransaction(sig);
    await program.methods.lockStake(new BN(500)).accounts({{ farm, staker: staker.publicKey }}).signers([staker]).rpc();
    await program.methods
      .updateRates(new BN(50))
      .accounts({{ farm, authority: admin.publicKey }})
      .rpc();
    const before = await provider.connection.getBalance(staker.publicKey);
    await program.methods.claimYield().accounts({{ farm, staker: staker.publicKey }}).signers([staker]).rpc();
    const after = await provider.connection.getBalance(staker.publicKey);
    assert.isAbove(after, before);
  }});
}});
"#
    )
}

#[test]
fn f1_tp_wallet_admin_running_the_gated_update_still_fires_rg03() {
    let src = wallet_admin_updates_rates();
    let h = hits("tp-wallet-admin", &src);
    assert!(!fired(&h, "RG03").is_empty(), "{h:?}");
    // The generated staker has no role: RG02 stays silent on its steps.
    assert!(fired(&h, "RG02").is_empty(), "{h:?}");
}

// ---------------------------------------------------------------------------
// Determinism

#[test]
fn f1_cli_output_is_byte_identical_across_runs() {
    let src = registered_moderator();
    let (_, a) = run_cli("det-a", "poc.ts", &src, false);
    let (_, b) = run_cli("det-b", "poc.ts", &src, false);
    assert_eq!(a, b);
    let frag = fragment_pledge_reclaim("");
    let (_, c) = run_cli("det-c", "poc.ts", &frag, true);
    let (_, d) = run_cli("det-d", "poc.ts", &frag, true);
    assert_eq!(c, d);
}

// ---------------------------------------------------------------------------
// Property: removing a binding never adds a flag

/// The binding lines a generated test may carry, each one evidence that a
/// holder is bound to an actor that is not the attacker.
const BINDINGS: &[&str] = &[
    // The staker is loaded rather than generated.
    "    const staker = Keypair.fromSecretKey(Uint8Array.from(STAKER_SECRET));",
    // The farm admin (a default signer with the admin role) gives the staker a tier.
    "    await program.methods.setStakerTier(1).accounts({ farm, authority: admin.publicKey, staker: staker.publicKey }).rpc();",
    // The admin is initialised as the farm's admin before the exploit.
    "    await program.methods.initialize(new BN(10)).accounts({ farm, admin: admin.publicKey }).rpc();",
];

fn property_source(present: &[bool], exploit_by_admin: bool) -> String {
    let mut setup = String::new();
    if present[0] {
        setup.push_str(BINDINGS[0]);
    } else {
        setup.push_str("    const staker = Keypair.generate();");
    }
    setup.push('\n');
    if present[2] {
        setup.push_str(BINDINGS[2]);
        setup.push('\n');
    }
    if present[1] {
        setup.push_str(BINDINGS[1]);
        setup.push('\n');
    }
    let exploit = if exploit_by_admin {
        "    await program.methods.updateRates(new BN(50)).accounts({ farm, authority: admin.publicKey }).rpc();"
    } else {
        "    await program.methods.claimYield().accounts({ farm, staker: staker.publicKey }).signers([staker]).rpc();"
    };
    format!(
        r#"{TS_HEAD}
describe("property", () => {{
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);
  const program = anchor.workspace.Yieldfarm as Program<any>;
  const admin = (provider.wallet as anchor.Wallet).payer;

  it("exploit", async () => {{
    const [farm] = PublicKey.findProgramAddressSync([Buffer.from("farm")], program.programId);
{setup}    await program.methods.lockStake(new BN(500)).accounts({{ farm, staker: staker.publicKey }}).signers([staker]).rpc();
    const before = await provider.connection.getBalance(staker.publicKey);
{exploit}
    const after = await provider.connection.getBalance(staker.publicKey);
    assert.isAbove(after, before);
  }});
}});
"#
    )
}

fn privilege_flags(src: &str) -> Vec<(String, u32)> {
    let p = Project::from_sources(&[("tests/property.ts", src)]);
    let opts = Options {
        program_model: Some(model().clone()),
        ..Options::default()
    };
    let r = analyze(&p, None, &opts);
    assert!(r.parse_errors.is_empty(), "{:?}", r.parse_errors);
    r.active()
        .filter(|f| f.rule == "RG02" || f.rule == "RG03")
        .map(|f| (f.rule.to_string(), f.line))
        .collect()
}

/// Line numbers shift when a binding line is removed; compare RG02 by rule
/// and by the line's text instead. RG03 reports once per test, at the first
/// exploit step it applies to, so it is compared by presence: a binding line
/// that is itself an exploit step can move that one report without adding one.
fn flags_by_text(src: &str) -> Vec<(String, String)> {
    let lines: Vec<&str> = src.lines().collect();
    let mut out: Vec<(String, String)> = privilege_flags(src)
        .into_iter()
        .map(|(rule, line)| {
            let text = if rule == "RG03" {
                String::new()
            } else {
                lines
                    .get(line as usize - 1)
                    .map(|l| l.trim().to_string())
                    .unwrap_or_default()
            };
            (rule, text)
        })
        .collect();
    out.sort();
    out.dedup();
    out
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 48, ..ProptestConfig::default() })]

    #[test]
    fn f1_removing_a_binding_never_adds_a_flag(
        present in proptest::collection::vec(any::<bool>(), 3),
        drop in 0usize..3,
        exploit_by_admin in any::<bool>(),
    ) {
        prop_assume!(present[drop]);
        let with = property_source(&present, exploit_by_admin);
        let mut fewer = present.clone();
        fewer[drop] = false;
        let without = property_source(&fewer, exploit_by_admin);
        let flags_with = flags_by_text(&with);
        let flags_without = flags_by_text(&without);
        for flag in &flags_without {
            prop_assert!(
                flags_with.contains(flag),
                "removing binding {drop} added {flag:?}\nwith: {flags_with:?}\nwithout: {flags_without:?}"
            );
        }
    }
}

#[test]
fn f1_the_property_is_not_vacuous() {
    // With every binding present, both exploits are flagged: the claim by the
    // bound staker (RG02) and the update by the admin wallet (RG03). Without
    // any binding, neither is.
    let all = [true, true, true];
    assert!(
        flags_by_text(&property_source(&all, false))
            .iter()
            .any(|(r, _)| r == "RG02"),
        "{:?}",
        flags_by_text(&property_source(&all, false))
    );
    assert!(
        flags_by_text(&property_source(&all, true))
            .iter()
            .any(|(r, _)| r == "RG03"),
        "{:?}",
        flags_by_text(&property_source(&all, true))
    );
    let none = [false, false, false];
    assert!(flags_by_text(&property_source(&none, false)).is_empty());
    assert!(flags_by_text(&property_source(&none, true)).is_empty());
}
