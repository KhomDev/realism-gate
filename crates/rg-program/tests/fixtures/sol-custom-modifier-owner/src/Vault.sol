pragma solidity ^0.8.20;

contract Vault {
    address public owner;

    modifier guarded() {
        require(msg.sender == owner, "no");
        _;
    }

    function close() external guarded {
    }
}
