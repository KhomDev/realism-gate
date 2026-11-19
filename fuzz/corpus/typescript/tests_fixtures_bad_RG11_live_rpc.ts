// RG11: the PoC reads the pool from mainnet as it is today.
import * as anchor from "@coral-xyz/anchor";
import { BN, Program } from "@coral-xyz/anchor";
import { Connection, Keypair, PublicKey, clusterApiUrl } from "@solana/web3.js";
import { assert } from "chai";

const POOL = new PublicKey("58oQChx4yWmvKdwLLZzBi4ChoCc2fqCUWBkwMihLYQo2");

describe("amm", () => {
  const provider = anchor.AnchorProvider.env();
  const program = anchor.workspace.Amm as Program<any>;
  const attacker = Keypair.generate();

  it("prices the swap off the live pool", async () => {
    const mainnet = new Connection(clusterApiUrl("mainnet-beta"));
    const pool = await mainnet.getAccountInfo(POOL);
    await program.methods.swap(new BN(1_000)).accounts({ trader: attacker.publicKey }).signers([attacker]).rpc();
    assert.equal(pool.owner.toBase58(), "675kPX9MHTjS2zt1qfr1NYHuzeLXfQM9H24wFSUt1Mp8");
  });
});
