// SPDX-License-Identifier: MIT
pragma solidity ^0.8.13;

import './SafeMath.sol';

contract Main {
    function add(uint x, uint y) internal pure returns (uint) {
        uint z = x + y;
        require(z >= x, "uint overflow");

        return z;
    }
}
