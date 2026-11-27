// Twin: an it() block with no imports that names nothing only a Solana test
// spells. It is not a Solana test and must stay unread.
it("adds two numbers", () => {
  const total = add(2, 3);
  expect(total).to.equal(5);
});
