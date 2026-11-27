pragma solidity ^0.8.20;

contract Vault {
    function owner() public view returns (address) {
        return address(0);
    }

    function close() external {
        require(msg.sender == owner(), "not owner");
    }
}
