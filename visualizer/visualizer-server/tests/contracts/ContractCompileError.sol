// SPDX-License-Identifier: MIT
pragma solidity ^0.8.13;

contract Main {
    uint public C = 0;
    address a = 0x5C69bEe701ef814a2B6a3EDD4B1652CB9cc5aA6f;
    // a compilation error occurs here because it is not possible to add variables
    // with such types, but sol2uml will ignore it
    uint public error = C + a;

    function add(uint x, address y) internal pure returns (uint) {
        // a compilation error occurs here because it is not possible to add variables
        // with such types, but sol2uml will ignore it
        uint z = x + y;
        require(z >= x, "uint overflow");

        return z;
    }
}
