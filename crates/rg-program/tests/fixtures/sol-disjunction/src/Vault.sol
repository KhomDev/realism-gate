pragma solidity ^0.8.20;

contract Vault {
    address public owner;
    address admin;

    function close() external {
        require(msg.sender == owner || msg.sender == admin, "no");
    }
}
