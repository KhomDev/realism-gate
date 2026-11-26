//! F2: the Solidity rule path reads the RG-A1 program model of
//! `--model-target`.
//!
//! RG03 takes "is this call access-controlled" from the model when it has an
//! answer (`onlyOwner` on a function whose name says nothing), and RG02 drops
//! a role-read prank whose every call the model resolves to a function that
//! requires no caller authority. Without a model both rules are unchanged
//! (the golden manifest pins that byte for byte). The contracts are
//! synthetic, written for this file.

use realism_gate::engine::{Options, Report, analyze};
use realism_gate::project::Project;
use realism_gate::Severity;

fn scratch(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("rg-f2-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("src")).unwrap();
    dir
}

/// A target checkout with one `Vault`. `harvest_modifier` and
/// `deposit_modifier` are written after the function's parameter list.
fn vault_model(name: &str, harvest_modifier: &str, deposit_modifier: &str) -> rg_program::ProgramModel {
    let dir = scratch(name);
    std::fs::write(
        dir.join("src/Vault.sol"),
        format!(
            r#"// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

contract Vault {{
    address public owner;
    mapping(address => uint256) public shares;

    constructor() {{ owner = msg.sender; }}

    modifier onlyOwner() {{
        require(msg.sender == owner, "not owner");
        _;
    }}

    function harvest() external {harvest_modifier} {{}}

    function deposit() external payable {deposit_modifier} {{
        shares[msg.sender] += msg.value;
    }}
}}
"#
        ),
    )
    .unwrap();
    let model = rg_program::ProgramModel::resolve(&dir).expect("the Vault resolves");
    let _ = std::fs::remove_dir_all(&dir);
    model
}

/// The test contract deploys the vault, so it is the vault's owner, and then
/// calls `harvest`, a name that does not read as access-controlled.
const DEPLOYER_HARVESTS: &str = r#"// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;
import "forge-std/Test.sol";
import "../src/Vault.sol";

contract HarvestTest is Test {
    Vault vault;

    function setUp() public {
        vault = new Vault();
    }

    function testHarvestDrains() public {
        uint256 before = address(this).balance;
        vault.harvest();
        assertGt(address(this).balance, before);
    }
}
"#;

/// The exploit pranks as `admin` (a role by name) to call `deposit`.
const ADMIN_DEPOSITS: &str = r#"// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;
import "forge-std/Test.sol";
import "../src/Vault.sol";

contract DepositTest is Test {
    Vault vault;
    address admin = makeAddr("admin");

    function setUp() public {
        vm.prank(admin);
        vault = new Vault();
        vm.deal(admin, 1 ether);
    }

    function testAdminDeposit() public {
        uint256 before = vault.shares(admin);
        vm.prank(admin);
        vault.deposit{value: 1 ether}();
        assertGt(vault.shares(admin), before);
    }
}
"#;

fn report(src: &str, model: Option<&rg_program::ProgramModel>) -> Report {
    let p = Project::from_sources(&[("test/Vault.t.sol", src)]);
    let opts = Options {
        program_model: model.cloned(),
        ..Options::default()
    };
    let r = analyze(&p, None, &opts);
    assert!(r.parse_errors.is_empty(), "{:?}", r.parse_errors);
    r
}

fn rule<'r>(r: &'r Report, id: &str) -> Vec<&'r realism_gate::finding::Finding> {
    r.active().filter(|f| f.rule == id).collect()
}

#[test]
fn f2_rg03_reads_only_owner_from_the_model() {
    // Without a model, `harvest` is not named like a gated call: RG03 is the
    // ordinary local-deployment note.
    let plain = report(DEPLOYER_HARVESTS, None);
    let rg03 = rule(&plain, "RG03");
    assert_eq!(rg03.len(), 1, "{:?}", plain.findings);
    assert_eq!(rg03[0].severity, Severity::Note);
    // The model says `harvest` is `onlyOwner`: the deployer is using its
    // owner privilege, and RG03 says so at warning level.
    let model = vault_model("only-owner", "onlyOwner", "");
    let with = report(DEPLOYER_HARVESTS, Some(&model));
    let rg03 = rule(&with, "RG03");
    assert_eq!(rg03.len(), 1, "{:?}", with.findings);
    assert_eq!(rg03[0].severity, Severity::Warn, "{}", rg03[0].message);
    assert!(rg03[0].message.contains("harvest"), "{}", rg03[0].message);
}

#[test]
fn f2_rg03_model_without_a_modifier_keeps_the_note() {
    let model = vault_model("no-modifier", "", "");
    let with = report(DEPLOYER_HARVESTS, Some(&model));
    let rg03 = rule(&with, "RG03");
    assert_eq!(rg03.len(), 1, "{:?}", with.findings);
    assert_eq!(rg03[0].severity, Severity::Note);
}

#[test]
fn f2_rg02_drops_a_role_prank_whose_calls_need_no_authority() {
    // Without a model the prank as `admin` is reported by its name.
    let plain = report(ADMIN_DEPOSITS, None);
    assert_eq!(rule(&plain, "RG02").len(), 1, "{:?}", plain.findings);
    // The model resolves `deposit` to a function anyone may call: signing as
    // `admin` proves nothing an attacker could not do from its own address.
    let open = vault_model("deposit-open", "", "");
    let with = report(ADMIN_DEPOSITS, Some(&open));
    assert!(rule(&with, "RG02").is_empty(), "{:?}", with.findings);
}

#[test]
fn f2_rg02_still_fires_when_the_model_gates_the_call() {
    let gated = vault_model("deposit-gated", "", "onlyOwner");
    let with = report(ADMIN_DEPOSITS, Some(&gated));
    assert_eq!(rule(&with, "RG02").len(), 1, "{:?}", with.findings);
}

#[test]
fn f2_the_cli_passes_the_model_to_the_solidity_rules() {
    // End to end through the binary: `--model-target` must reach the
    // Solidity path (the argument dropped in engine.rs fails this test).
    let dir = std::env::temp_dir().join(format!("rg-f2-{}-cli", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("target/src")).unwrap();
    std::fs::create_dir_all(dir.join("poc/test")).unwrap();
    std::fs::write(
        dir.join("target/src/Vault.sol"),
        "// SPDX-License-Identifier: MIT\npragma solidity ^0.8.20;\ncontract Vault {\n    address public owner;\n    constructor() { owner = msg.sender; }\n    modifier onlyOwner() { require(msg.sender == owner, \"no\"); _; }\n    function harvest() external onlyOwner {}\n}\n",
    )
    .unwrap();
    std::fs::write(dir.join("poc/test/Vault.t.sol"), DEPLOYER_HARVESTS).unwrap();
    let run = |model: bool| {
        let mut args = vec![
            "check",
            "test/Vault.t.sol",
            "--no-declaration",
            "--no-config",
            "--format",
            "json",
            "--fail-on",
            "never",
        ];
        if model {
            args.extend(["--model-target", "../target"]);
        }
        let out = std::process::Command::new(env!("CARGO_BIN_EXE_realism-gate"))
            .args(&args)
            .current_dir(dir.join("poc"))
            .env("NO_COLOR", "1")
            .output()
            .expect("the binary runs");
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
        out.stdout
    };
    let severity = |stdout: &[u8]| -> String {
        let v: serde_json::Value = serde_json::from_slice(stdout).unwrap();
        v["findings"]
            .as_array()
            .unwrap()
            .iter()
            .find(|f| f["rule"] == "RG03")
            .map(|f| f["severity"].as_str().unwrap().to_string())
            .unwrap_or_default()
    };
    let without = run(false);
    let with = run(true);
    assert_eq!(severity(&without), "note");
    assert_eq!(severity(&with), "warn");
    // Determinism: the same run twice is byte-identical.
    assert_eq!(with, run(true));
    let _ = std::fs::remove_dir_all(&dir);
}
