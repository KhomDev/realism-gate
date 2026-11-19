// RG06: the vault's token account is written with a balance it never
// received.
import { LiteSVM } from "litesvm";
import { ACCOUNT_SIZE, AccountLayout, TOKEN_PROGRAM_ID, getAssociatedTokenAddressSync } from "@solana/spl-token";
import { Keypair, LAMPORTS_PER_SOL, PublicKey, Transaction } from "@solana/web3.js";
import { assert } from "chai";

const PROGRAM_ID = new PublicKey("Vau1t11111111111111111111111111111111111111");
const USDC = new PublicKey("EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v");

test("withdraws more than was deposited", () => {
  const svm = new LiteSVM();
  svm.addProgramFromFile(PROGRAM_ID, "target/deploy/vault.so");
  const attacker = Keypair.generate();
  svm.airdrop(attacker.publicKey, BigInt(LAMPORTS_PER_SOL));
  const [vault] = PublicKey.findProgramAddressSync([Buffer.from("vault")], PROGRAM_ID);
  const vaultTokens = getAssociatedTokenAddressSync(USDC, vault, true);

  const data = Buffer.alloc(ACCOUNT_SIZE);
  AccountLayout.encode({ mint: USDC, owner: vault, amount: 1_000_000_000n, state: 1 } as any, data);
  svm.setAccount(vaultTokens, { lamports: 2_039_280, data, owner: TOKEN_PROGRAM_ID, executable: false });

  const tx = new Transaction().add(withdrawIx(attacker.publicKey, vaultTokens));
  tx.recentBlockhash = svm.latestBlockhash();
  tx.sign(attacker);
  svm.sendTransaction(tx);
  const left = AccountLayout.decode(svm.getAccount(vaultTokens)!.data);
  assert.equal(left.amount, 0n);
});
