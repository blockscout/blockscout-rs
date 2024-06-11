import { ByteArray, Bytes, crypto } from "@graphprotocol/graph-ts";

export function byteArrayFromHex(s: string): ByteArray {
  if (s.length % 2 !== 0) {
    throw new TypeError("Hex string must have an even number of characters");
  }
  let out = new Uint8Array(s.length / 2);
  for (var i = 0; i < s.length; i += 2) {
    out[i / 2] = parseInt(s.substring(i, i + 2), 16) as u32;
  }
  return changetype<ByteArray>(out);
}

export function hashByName(name: string): ByteArray {
  if (!name) {
    return byteArrayFromHex("0".repeat(64));
  } else {
    const partition = splitStringOnce(name, ".");
    const label = partition[0];
    const remainder = partition[1];

    return crypto.keccak256(
      concat(hashByName(remainder), keccakFromStr(label))
    );
  }
}

function splitStringOnce(input: string, separator: string): string[] {
  const splitArray = input.split(separator, 2);

  if (splitArray.length === 2) {
    return [splitArray[0], splitArray[1]];
  } else {
    return [input, ""];
  }
}

export function concat(a: ByteArray, b: ByteArray): ByteArray {
  let out = new Uint8Array(a.length + b.length);
  for (let i = 0; i < a.length; i++) {
    out[i] = a[i];
  }
  for (let j = 0; j < b.length; j++) {
    out[a.length + j] = b[j];
  }
  // return out as ByteArray
  return changetype<ByteArray>(out);
}

function keccakFromStr(s: string): ByteArray {
  return crypto.keccak256(Bytes.fromUTF8(s));
}
