import * as anchor from "@coral-xyz/anchor";
import { Keypair } from "@solana/web3.js";

// DEV class "fragments that fail to parse", mixed-language variant.
// Synthetic: a TypeScript test saved under a .rs name. It must be reported
// as unreadable, with the language named as the reason.
describe("vault", () => {
  it("attacker withdraws", async () => {
    const attacker = Keypair.generate();
    await anchor.workspace.Vault.methods.withdraw().signers([attacker]).rpc();
  });
});
