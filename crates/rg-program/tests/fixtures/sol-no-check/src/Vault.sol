pragma solidity ^0.8.20;

contract Vault {
    uint256 public total;

    function open() external {
        total += 1;
    }
}
