pragma solidity ^0.8.20;

contract Base {
    address baseAdmin;

    modifier gate() {
        require(msg.sender == baseAdmin, "no");
        _;
    }
}

contract Vault is Base {
    address vaultAdmin;

    modifier gate() {
        require(msg.sender == vaultAdmin, "no");
        _;
    }

    function close() external gate {
    }
}
