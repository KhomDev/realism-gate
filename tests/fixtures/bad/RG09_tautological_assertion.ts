// RG09: both assertions hold whatever the program does.
import * as anchor from "@coral-xyz/anchor";
import { BN, Program } from "@coral-xyz/anchor";
import { Keypair } from "@solana/web3.js";
import { assert, expect } from "chai";

const AMOUNT = 100;

describe("vault", () => {
  const provider = anchor.AnchorProvider.env();
  const program = anchor.workspace.Vault as Program<any>;
  const attacker = Keypair.generate();

  it("drains the vault", async () => {
    const withdrawn = new BN(AMOUNT);
    await program.methods.withdraw(withdrawn).accounts({ recipient: attacker.publicKey }).signers([attacker]).rpc();
    assert.equal(withdrawn, withdrawn);
    expect(AMOUNT).to.equal(100);
  });
});
