pragma solidity ^0.8.20;

contract Vault {
    bytes32 constant MINTER_ROLE = keccak256("MINTER_ROLE");

    function mint() external onlyRole(MINTER_ROLE) {
    }
}
