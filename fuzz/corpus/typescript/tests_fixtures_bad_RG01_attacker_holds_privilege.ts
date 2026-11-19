// RG01: the PoC initializes the vault with the attacker as its admin, then
// "exploits" the admin's own setter.
import * as anchor from "@coral-xyz/anchor";
import { BN, Program } from "@coral-xyz/anchor";
import { Keypair, PublicKey } from "@solana/web3.js";
import { assert } from "chai";

describe("vault", () => {
  const provider = anchor.AnchorProvider.env();
  const program = anchor.workspace.Vault as Program<any>;
  const attacker = Keypair.generate();
  const [vault] = PublicKey.findProgramAddressSync([Buffer.from("vault")], program.programId);

  it("sets the fee to 100%", async () => {
    await program.methods
      .initialize()
      .accounts({ vault, admin: attacker.publicKey })
      .rpc();
    await program.methods
      .setFee(new BN(10_000))
      .accounts({ vault, admin: attacker.publicKey })
      .signers([attacker])
      .rpc();
    const state = await program.account.vault.fetch(vault);
    assert.equal(state.feeBps, 10_000);
  });
});
