import { assert, describe, test } from "matchstick-as/assembly/index";
import { checkValidLabel } from "../src/utils";

describe("checkValidLabel()", () => {
  test("returns false for null byte in label", () => {
    const label = "example\0";
    assert.assertTrue(checkValidLabel(label) === false);
  });
  test("returns false for '.' in label", () => {
    const label = "example.123";
    assert.assertTrue(checkValidLabel(label) === false);
  });
  test("returns false for '[' in label", () => {
    const label = "[example123";
    assert.assertTrue(checkValidLabel(label) === false);
  });
  test("returns false for ']' in label", () => {
    const label = "example123]";
    assert.assertTrue(checkValidLabel(label) === false);
  });
  test("returns true for normal label", () => {
    const label = "example123";
    assert.assertTrue(checkValidLabel(label) === true);
  });
});
