// SPDX-License-Identifier: MIT

pragma solidity ^0.8.0;

import "./A.sol";

contract B is A {
    function number() internal view virtual override returns (uint16) {
        return 123;
    }
}