import {
  describe,
  assert,
  test,
} from "matchstick-as/assembly/index"
import { hashByName } from "../src/utils"
import { Bytes } from "@graphprotocol/graph-ts"

describe("Utils", () => {
  test("Name hashing works", () => {
    assert.bytesEquals(
      Bytes.fromHexString('0xd1b419b672a0a0f45d8b7d8e7c7b80d56f1ba5d703ea1d37424eb7e1d82bc620'),
      Bytes.fromByteArray(hashByName('levvv'))
    )

    assert.bytesEquals(
      Bytes.fromHexString('0x38a7804a53792b0cdefe3e7271b0b85422d620ea4a82df7b7bf750a6d4b297a4'),
      Bytes.fromByteArray(hashByName('levvv.eth'))
    )
  })
});
