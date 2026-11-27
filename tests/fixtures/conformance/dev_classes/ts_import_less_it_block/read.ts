// DEV class "the file-level Solana import gate rejects an it() block".
// Synthetic: an it() block pasted into an issue with no imports. It must be
// read as a Solana test.
it("attacker withdraws from a vault it does not own", async () => {
  const attacker = Keypair.generate();
  const before = await provider.connection.getBalance(attacker.publicKey);
  await program.methods
    .withdraw(new anchor.BN(5))
    .accounts({ user: attacker.publicKey })
    .signers([attacker])
    .rpc();
  const after = await provider.connection.getBalance(attacker.publicKey);
  assert.isAbove(after, before);
});
