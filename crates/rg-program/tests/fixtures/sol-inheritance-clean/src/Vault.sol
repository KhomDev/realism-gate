pragma solidity ^0.8.20;

contract Base {
    function pause() external onlyOwner {
    }
}

contract Vault is Base {
    function open() external {
    }
}
