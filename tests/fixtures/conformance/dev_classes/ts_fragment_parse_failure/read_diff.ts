// DEV class "fragments that fail to parse", diff variant. Synthetic: the
// author pasted the test as a unified diff against the suite.
  it("sets a limit", async () => {
    await program.methods.setLimit(new anchor.BN(5)).accounts({ admin: admin.publicKey }).signers([admin]).rpc();
  });

+ it("attacker resets the limit", async () => {
+   const attacker = Keypair.generate();
-   await program.methods.setLimit(new anchor.BN(5)).rpc();
+   await program.methods.setLimit(new anchor.BN(9)).accounts({ admin: attacker.publicKey }).signers([attacker]).rpc();
++  const state = await program.account.config.fetch(configPda);
+   assert.equal(state.limit.toNumber(), 9);
+ });
