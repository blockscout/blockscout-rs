import {
  describe,
  test,
} from "matchstick-as/assembly/index"
import { byteArrayFromHex, hashNyName } from "../src/utils"

describe("Describe entity assertions", () => {

  test("hashByName works", () => {
    assert(byteArrayFromHex('d1b419b672a0a0f45d8b7d8e7c7b80d56f1ba5d703ea1d37424eb7e1d82bc620').equals(
      hashNyName('levvv')
    ))
    assert(byteArrayFromHex('38a7804a53792b0cdefe3e7271b0b85422d620ea4a82df7b7bf750a6d4b297a4').equals(
      hashNyName('levvv.eth')
    ))
    
  })
})
