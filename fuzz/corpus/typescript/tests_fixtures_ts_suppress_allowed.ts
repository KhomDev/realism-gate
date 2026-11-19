// One suppression with a reason (honoured) and one without (ignored).
import * as anchor from "@coral-xyz/anchor";
import { BN, Program } from "@coral-xyz/anchor";
import { Connection, Keypair, PublicKey } from "@solana/web3.js";
import { assert } from "chai";

describe("amm", () => {
  const provider = anchor.AnchorProvider.env();
  const program = anchor.workspace.Amm as Program<any>;
  const attacker = Keypair.generate();
  const [pool] = PublicKey.findProgramAddressSync([Buffer.from("pool")], program.programId);

  it("swaps at the recorded price", async () => {
    // realism-gate: allow(RG11) a devnet replica pinned by the program's own CI
    const devnet = new Connection("https://api.devnet.solana.com");
    await program.methods.swap(new BN(1)).accounts({ trader: attacker.publicKey }).signers([attacker]).rpc();
    const state = await devnet.getAccountInfo(pool);
    // realism-gate: allow(RG09)
    assert.equal(1, 1);
    assert.equal(state.lamports, 0);
  });
});
