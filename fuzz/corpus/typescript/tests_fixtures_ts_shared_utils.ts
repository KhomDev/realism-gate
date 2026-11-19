// Helpers shared by the tests in this directory.
import { BN, Program } from "@coral-xyz/anchor";
import { Keypair } from "@solana/web3.js";

export async function withdrawAs(program: Program<any>, signer: Keypair, amount: number) {
  await program.methods
    .withdraw(new BN(amount))
    .accounts({ recipient: signer.publicKey })
    .signers([signer])
    .rpc();
}
