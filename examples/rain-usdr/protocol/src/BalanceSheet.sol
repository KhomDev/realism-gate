// SPDX-License-Identifier: MIT
pragma solidity 0.8.24;

/// The system's books. distributeSurplus assumes every unit of fee-exempt
/// collateral debt has a reserve counterpart: the PSM put stablecoins in the
/// reserve, and the debt is the bookkeeping mirror of them.
contract BalanceSheet {
    error ReserveBackingShortfall(uint256 reserve, uint256 backed);

    /// Stablecoins the PSM has put in the reserve, plus accumulated surplus.
    uint256 internal reserve_;
    /// Debt of every fee-exempt collateral type: PSM deposits and, wrongly,
    /// draws from vaults on a fee-exempt type.
    uint256 internal feeExemptDebt_;

    address public engine;
    address public psm;

    function setModules(address engine_, address psm_) external {
        engine = engine_;
        psm = psm_;
    }

    function reserve() external view returns (uint256) {
        return reserve_;
    }

    function feeExemptDebt() external view returns (uint256) {
        return feeExemptDebt_;
    }

    /// Called by the engine on a draw and by the PSM on a deposit. Both write
    /// the same counter, which is why a self-backed draw can consume the
    /// PSM's headroom.
    function addDebt(uint256 amount) external {
        require(msg.sender == engine || msg.sender == psm, "not a module");
        feeExemptDebt_ += amount;
    }

    /// A keeper adds the stablecoins bought with the surplus.
    function addReserve(uint256 amount) external {
        reserve_ += amount;
    }

    /// The check runs before the "nothing to distribute" early return, so a
    /// self-backed vault's debt halts every call.
    function distributeSurplus() external returns (uint256 shipped) {
        if (reserve_ < feeExemptDebt_) {
            revert ReserveBackingShortfall(reserve_, feeExemptDebt_);
        }
        shipped = reserve_ - feeExemptDebt_;
        if (shipped == 0) {
            return 0;
        }
        reserve_ = feeExemptDebt_;
    }
}
