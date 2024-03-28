import {
  describe,
  test,
} from "matchstick-as/assembly/index"
import { byteArrayFromHex, hashNyName } from "../src/utils"

describe("Describe entity assertions", () => {

  test("hashByName works", () => {
    assert(byteArrayFromHex('24343f82ad351b7c6160d3d88e9190179a6840958c45b433731d5b3e18df40fc').equals(
      hashNyName('alice')
    ))
    assert(byteArrayFromHex('7fdf67417cd18098194f331e7df5b839e400fd37e0607276acfeea6959fb4e31').equals(
      hashNyName('alice.pls')
    ))
    
  })
})
