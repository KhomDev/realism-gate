// SPDX-License-Identifier: MIT
pragma solidity 0.8.24;

import {BalanceSheet} from "./BalanceSheet.sol";
import {Stablecoin} from "./mocks/Stablecoin.sol";

/// The Peg Stability Module. Its deposits are the only route the fee-exempt
/// collateral types were meant to be used through. The ceiling counts every
/// unit of fee-exempt debt, including a self-backed vault's.
contract PegStabilityModule {
    error CeilingExceeded(uint256 ceiling, uint256 used);

    Stablecoin public immutable stable;
    BalanceSheet public immutable sheet;
    uint256 public immutable ceiling;

    constructor(address stable_, address sheet_, uint256 ceiling_) {
        stable = Stablecoin(stable_);
        sheet = BalanceSheet(sheet_);
        ceiling = ceiling_;
    }

    function used() external view returns (uint256) {
        return sheet.feeExemptDebt();
    }

    function sellStable(address usr, uint256 amount) external returns (uint256) {
        uint256 used_ = sheet.feeExemptDebt();
        if (used_ + amount > ceiling) {
            revert CeilingExceeded(ceiling, used_);
        }
        sheet.addDebt(amount);
        stable.transferFrom(msg.sender, address(this), amount);
        return amount;
    }
}
