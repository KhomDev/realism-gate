pragma solidity ^0.8.20;

contract Vault {
    address public owner;

    function close() external {
        if (msg.sender != owner) {
            revert();
        }
    }
}
