pragma solidity ^0.8.20;

contract Vault {
    address admin;

    modifier onlyAdmin() {
        require(msg.sender == admin, "not admin");
        _;
    }

    function pause() external onlyAdmin {
    }
}
