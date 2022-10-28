// SPDX-License-Identifier: MIT
// source: https://solidity-by-example.org/app/erc20/

pragma solidity ^0.8.13;

import "./openzeppelin_contracts/ERC20.sol";
import "./useless_libraries/SafeMath1.sol";
import "./useless_libraries/SafeMath2.sol";
import "./useless_libraries/SafeMath3.sol";
import "./useless_libraries/SafeMath4.sol";
import "./useless_libraries/SafeMath5.sol";

contract MyToken is ERC20 {
    // now 'add' method is not unique for uint and can`t be used but it isn`t a problem for this test
    using SafeMath1 for uint;
    using SafeMath2 for uint;
    using SafeMath3 for uint;
    using SafeMath4 for uint;
    using SafeMath5 for uint;

    constructor(string memory name, string memory symbol) ERC20() {
        // Mint 100 tokens to msg.sender
        // Similar to how
        // 1 dollar = 100 cents
        // 1 token = 1 * (10 ** decimals)
        _mint(100 * 10**decimals);
    }
}
