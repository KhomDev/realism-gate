// The suite's own helper module: the only file that imports Anchor.
import * as anchor from "@coral-xyz/anchor";
import { Keypair } from "@solana/web3.js";

export const provider = anchor.AnchorProvider.env();
export const program = anchor.workspace.Vault;
export const newUser = () => Keypair.generate();
