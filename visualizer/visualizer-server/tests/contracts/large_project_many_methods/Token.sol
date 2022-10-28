// SPDX-License-Identifier: MIT
// source: https://solidity-by-example.org/app/erc20/

pragma solidity ^0.8.13;

import "./openzeppelin_contracts/ERC20.sol";

contract MyToken is ERC20 {
    uint public useless_variable1 = 1;
    uint32 public useless_variable2 = 1;
    uint32 private useless_variable3 = 1;
    uint112 public useless_variable4;
    uint256 public useless_variable5;
    bool public useless_variable6 = true;
    address public useless_variable7;
    uint public useless_variable8 = 1;

    constructor(string memory name, string memory symbol) ERC20() {
        // Mint 100 tokens to msg.sender
        // Similar to how
        // 1 dollar = 100 cents
        // 1 token = 1 * (10 ** decimals)
        _mint(100 * 10**decimals);
    }

    function uselessMethod1 (address recipient, uint amount) external returns (bool) {
        // nothing
        return true;
    }

    function uselessMethod2 (address recipient, bool param1, bool param2, bool param3, bool param4) public returns (bool) {
        // nothing
        return true;
    }

    function uselessMethod3 (address recipient, uint amount) external payable {
        // nothing
    }

    function uselessMethod4 (address recipient, uint amount) external returns (address) {
        // nothing
        return recipient;
    }

    function uselessMethod5 (address recipient, uint amount) external returns (bool) {
        // nothing
        return true;
    }

    function uselessMethod6 (address recipient, uint amount) external returns (bool) {
        // nothing
        return true;
    }

    function uselessMethod7 (address recipient, uint amount) external returns (bool) {
        // nothing
        return true;
    }

    function uselessMethod8 (address recipient, uint amount) external returns (bool) {
        // nothing
        return true;
    }
}
