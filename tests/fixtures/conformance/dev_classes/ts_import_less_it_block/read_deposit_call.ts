// DEV class "the file-level Solana import gate rejects an it() block",
// bare-statement variant. Synthetic: no it() at all, but the call
// `program.methods.deposit(` contains the text `it(`, so the file is not
// wrapped as a fragment before parsing. The frontend reads an import-less
// file's top level as one test.
const victim = Keypair.generate();
await program.methods.deposit(new anchor.BN(10)).accounts({ user: victim.publicKey }).signers([victim]).rpc();
const attacker = Keypair.generate();
await program.methods.withdraw(new anchor.BN(10)).accounts({ user: attacker.publicKey }).signers([attacker]).rpc();
assert.equal(await provider.connection.getBalance(attacker.publicKey), 10);
