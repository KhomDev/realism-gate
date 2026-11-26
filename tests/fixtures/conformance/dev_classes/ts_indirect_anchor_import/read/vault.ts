// DEV class "indirect Anchor import through utils.ts". Synthetic: the test
// file imports only its own ./utils and chai, so it imports no Solana package
// itself. It must be read.
import { program, provider, newUser } from "./utils";
import { assert } from "chai";

describe("vault", () => {
  it("attacker withdraws another user's deposit", async () => {
    const attacker = newUser();
    const before = await provider.connection.getBalance(attacker.publicKey);
    await program.methods
      .withdraw()
      .accounts({ user: attacker.publicKey })
      .signers([attacker])
      .rpc();
    const after = await provider.connection.getBalance(attacker.publicKey);
    assert.isAbove(after, before);
  });
});
