// DEV class "fragments that fail to parse", elision variant. Synthetic: the
// author elided the account lists and some statements with `...`. The
// fragment must be read.
it("attacker withdraws with elided accounts", async () => {
  const attacker = Keypair.generate();
  await program.methods.withdraw(new anchor.BN(1)).accounts({ user: attacker.publicKey, ... }).signers([attacker]).rpc();
  await program.methods.close().accounts({...}).rpc();
  ...
  assert.isTrue(true);
});
