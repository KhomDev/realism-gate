// Not a Solana test: an EVM suite in the same tree. It imports no Solana
// package, so it is not read at all, although it asserts nothing.
import { ethers } from "hardhat";

describe("Token", () => {
  it("deploys", async () => {
    const token = await ethers.deployContract("Token");
    await token.waitForDeployment();
    await ethers.provider.send("evm_mine", []);
  });
});
