pragma solidity ^0.8.20;

contract Vault {
    function close() external {
        require(isAllowed(msg.sender), "no");
    }

    function isAllowed(address) internal view returns (bool) {
        return true;
    }
}
