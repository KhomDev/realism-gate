pragma solidity ^0.8.20;

contract Vault {
    function open() external {
        require(msg.sender != address(0), "zero");
    }
}
