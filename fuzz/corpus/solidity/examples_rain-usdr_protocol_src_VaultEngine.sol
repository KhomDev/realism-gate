// SPDX-License-Identifier: MIT
pragma solidity 0.8.24;

import {BalanceSheet} from "./BalanceSheet.sol";

/// The vault engine. open() accepts any initialised collateral type, and frob()
/// applies its solvency gate only to volatile collateral, so a vault on a
/// fee-exempt stable type sits outside both that gate and the PSM's checks.
contract VaultEngine {
    struct Ilk {
        bool feeExempt;
        bool volatileCollateral;
        uint256 debtCeiling;
        uint256 rate;
    }

    mapping(bytes32 => Ilk) public ilks;
    mapping(address => mapping(bytes32 => uint256)) internal locked_;
    mapping(address => uint256) internal drawn_;

    BalanceSheet public immutable sheet;

    constructor(address sheet_) {
        sheet = BalanceSheet(sheet_);
    }

    function initIlk(bytes32 ilk, bool feeExempt, bool volatileCollateral, uint256 ceiling) external {
        ilks[ilk] = Ilk({
            feeExempt: feeExempt,
            volatileCollateral: volatileCollateral,
            debtCeiling: ceiling,
            rate: 1e27
        });
    }

    function locked(address usr, bytes32 ilk) external view returns (uint256) {
        return locked_[usr][ilk];
    }

    function drawn(address usr) external view returns (uint256) {
        return drawn_[usr];
    }

    /// The only checks are that the system is live, the owner non-zero and the
    /// rate set.
    function open(bytes32 ilk, address owner) external returns (uint256 vaultId) {
        require(ilks[ilk].rate != 0, "ilk not initialised");
        require(owner != address(0), "owner is zero");
        vaultId = uint256(uint160(owner)) ^ uint256(ilk);
    }

    /// Stable collateral is priced at par, so any user can join their own
    /// stablecoins and draw USDR 1:1.
    function join(address usr, uint256 amount) external {
        locked_[usr][keccak256("USDT-A")] += amount;
    }

    /// The solvency gate is applied only to volatile collateral.
    function frob(bytes32 ilk, uint256 dink, uint256 dart) external {
        Ilk memory i = ilks[ilk];
        require(i.rate != 0, "ilk not initialised");
        if (i.volatileCollateral) {
            require(locked_[msg.sender][ilk] >= dink, "not enough collateral");
        }
        require(drawn_[msg.sender] + dart <= i.debtCeiling, "ceiling");
        drawn_[msg.sender] += dart;
        locked_[msg.sender][ilk] += dink;
        sheet.addDebt(dart);
    }
}
