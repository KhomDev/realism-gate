// RG05: a mock oracle program is loaded at the real Pyth address.
import { Program } from "@coral-xyz/anchor";
import { BankrunProvider } from "anchor-bankrun";
import { startAnchor } from "solana-bankrun";
import { Keypair, PublicKey } from "@solana/web3.js";
import { assert } from "chai";
import IDL from "../target/idl/perps.json";

const PYTH = new PublicKey("FsJ3A3u2vn5cTVofAjvy6y5kwABJAqYWpe4975bi2epH");

describe("perps", () => {
  it("liquidates at a made-up price", async () => {
    const context = await startAnchor(
      "",
      [{ name: "mock_pyth", programId: PYTH }],
      [],
    );
    const program = new Program<any>(IDL, new BankrunProvider(context));
    const attacker = Keypair.generate();
    const [position] = PublicKey.findProgramAddressSync([Buffer.from("position")], program.programId);
    await program.methods.liquidate().accounts({ liquidator: attacker.publicKey, oracle: PYTH, position }).signers([attacker]).rpc();
    const state = await program.account.position.fetch(position);
    assert.isTrue(state.liquidated);
  });
});
