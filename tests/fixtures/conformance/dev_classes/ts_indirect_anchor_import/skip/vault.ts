// Twin: the same shape, but the suite imports Hardhat. It is an EVM test,
// whatever names it uses, and must stay unread.
import { ethers } from "hardhat";
import { program } from "./utils";
import { expect } from "chai";

describe("vault", () => {
  it("withdraws", async () => {
    const [user] = await ethers.getSigners();
    await program.methods.withdraw().accounts({ user: user.address }).rpc();
    expect(await ethers.provider.getBalance(user.address)).to.be.gt(0);
  });
});
