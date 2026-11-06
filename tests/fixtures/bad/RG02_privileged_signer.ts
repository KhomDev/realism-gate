// RG02: the admin swaps the oracle in the middle of the exploit.
import * as anchor from "@coral-xyz/anchor";
import { BN, Program } from "@coral-xyz/anchor";
import { Keypair, PublicKey } from "@solana/web3.js";
import { assert } from "chai";

describe("lending", () => {
  const provider = anchor.AnchorProvider.env();
  const program = anchor.workspace.Lending as Program<any>;
  const admin = Keypair.generate();
  const attacker = Keypair.generate();
  const priceFeed = Keypair.generate();
  const [position] = PublicKey.findProgramAddressSync(
    [Buffer.from("position"), attacker.publicKey.toBuffer()],
    program.programId,
  );

  before(async () => {
    await program.methods.initialize().accounts({ admin: admin.publicKey }).signers([admin]).rpc();
  });

  it("borrows against a moved price", async () => {
    await program.methods
      .setOracle(priceFeed.publicKey)
      .accounts({ admin: admin.publicKey })
      .signers([admin])
      .rpc();
    await program.methods
      .borrow(new BN(1_000))
      .accounts({ borrower: attacker.publicKey, position })
      .signers([attacker])
      .rpc();
    const state = await program.account.position.fetch(position);
    assert.equal(state.debt.toNumber(), 1_000);
  });
});
