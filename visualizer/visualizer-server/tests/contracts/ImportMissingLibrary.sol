// SPDX-License-Identifier: MIT
pragma solidity ^0.8.13;

// file with library doesn`t exist, so sol2uml will ignore it content but show variable 'v1' of library type
import './MissingLibrary.sol';

contract Main {
    using MissingLibrary for uint;

    function add(uint x, address y) internal pure returns (uint) {
        // a compilation error occurs here because it is not possible to add variables
        // with such types, but sol2uml will ignore it
        uint z = x + y;
        require(z >= x, "uint overflow");

        return z;
    }
}
