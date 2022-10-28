pragma solidity ^0.8.13;

import {A as A1} from "./Lib1.sol";
import {A as A2} from "./Lib2.sol";

contract A is A1, A2 {
    uint32 C1 = 0;
}
