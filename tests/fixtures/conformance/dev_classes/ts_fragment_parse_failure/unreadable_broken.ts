// Twin: a fragment that is broken for another reason (an unclosed call and a
// declaration with no name). No repair applies; it must report its error.
it("attacker withdraws", async () => {
  const attacker = Keypair.generate();
  await program.methods.withdraw(.accounts({ user: attacker.publicKey }).rpc();
  const = 5;
});
