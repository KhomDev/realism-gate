// RG08: one test asserts nothing, the other only prints.
import * as anchor from "@coral-xyz/anchor";
import { BN, Program } from "@coral-xyz/anchor";
import { Keypair } from "@solana/web3.js";

describe("vault", () => {
  const provider = anchor.AnchorProvider.env();
  const program = anchor.workspace.Vault as Program<any>;
  const attacker = Keypair.generate();

  it("drains the vault", async () => {
    await program.methods.withdraw(new BN(100)).accounts({ recipient: attacker.publicKey }).signers([attacker]).rpc();
  });

  it("prints the balance", async () => {
    await program.methods.withdraw(new BN(100)).accounts({ recipient: attacker.publicKey }).signers([attacker]).rpc();
    console.log("attacker", await provider.connection.getBalance(attacker.publicKey));
  });
});
