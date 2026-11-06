// RG03: the provider wallet sets the pool up and then "attacks" it.
import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { PublicKey } from "@solana/web3.js";
import { assert } from "chai";

describe("pool", () => {
  const provider = anchor.AnchorProvider.env();
  const program = anchor.workspace.Pool as Program<any>;
  const [pool] = PublicKey.findProgramAddressSync([Buffer.from("pool")], program.programId);

  it("sweeps the pool", async () => {
    await program.methods.initPool().accounts({ pool, payer: provider.wallet.publicKey }).rpc();
    await program.methods.sweep().accounts({ pool, recipient: provider.wallet.publicKey }).rpc();
    const state = await program.account.pool.fetch(pool);
    assert.equal(state.reserves.toNumber(), 0);
  });
});
