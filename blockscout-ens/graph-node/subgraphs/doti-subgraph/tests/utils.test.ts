import {
  describe,
  assert,
  test,
} from "matchstick-as/assembly/index";
import { hashByName } from "../src/utils";
import { Bytes } from "@graphprotocol/graph-ts";

describe("Utils", () => {
  test("Name hashing works for root tld", () => {
    assert.bytesEquals(
      Bytes.fromHexString('0x96b16e885d568c078028f8feef27a718c0e2a3cf42145b2100806cb1f07f4bb7'),
      Bytes.fromByteArray(hashByName('i'))
    );
  });

  test("Name hashing works for domain name", () => {
    let hashed = hashByName('test.i');
    assert.assertTrue(hashed.length == 32);
  });
});
