pragma solidity ^0.8.20;

contract Vault {
    bytes32 constant MINTER = keccak256("MINTER");

    function mint() external {
        require(hasRole(MINTER, msg.sender), "not minter");
    }

    function hasRole(bytes32, address) internal view returns (bool) {
        return true;
    }
}
