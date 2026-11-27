pragma solidity ^0.8.20;

contract Roles {
    bytes32 constant KEEPER = keccak256("KEEPER");

    modifier onlyKeeper() {
        require(hasRole(KEEPER, msg.sender), "no");
        _;
    }
}

contract Vault is Roles {
    function poke() external onlyKeeper {
    }
}
