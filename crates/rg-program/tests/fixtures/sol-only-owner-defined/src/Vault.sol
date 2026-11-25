pragma solidity ^0.8.20;

contract Vault {
    address public owner;

    modifier onlyOwner() {
        _checkOwner();
        _;
    }

    function _checkOwner() internal view {
        require(owner == msg.sender, "not owner");
    }

    function withdraw() external onlyOwner {
    }
}
