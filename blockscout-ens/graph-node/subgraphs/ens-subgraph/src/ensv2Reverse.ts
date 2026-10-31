// ENSIP-19 "default" (chain-agnostic) reverse resolution — DefaultReverseRegistrar
// is a single, pre-existing contract (not redeployed per contracts-v2
// release, unlike RootRegistry/ETHRegistry/etc.), so its address is a fixed
// per-network constant like the legacy v1 ENSRegistry's, not something
// sourced from contracts-v2/contracts/deployments/sepolia/*.json.
//
// Deliberately scoped to this one event/contract — see schema.graphql's
// comment on ENSv2PrimaryName for why the legacy addr.reverse scheme and
// other chains' L2ReverseRegistrar are out of scope here.
import { Bytes, crypto } from "@graphprotocol/graph-ts";
import { concat, createOrLoadAccount } from "./utils";

// Blockscout/BENS extension: namehash("addr.reverse") and the legacy reverse
// node for an address, namehash("<hex addr>.addr.reverse"), which is how v1
// consumers (BENS's addr_reverse_names view) key reverse lookups.
const ADDR_REVERSE_NODE = Bytes.fromHexString(
  "0x91d1777781884d03a6757a803996e38de2a42967fb37eeaca72729271025a9e2"
);
function legacyReverseNode(addr: Bytes): Bytes {
  let label = crypto.keccak256(Bytes.fromUTF8(addr.toHexString().slice(2)));
  return Bytes.fromByteArray(
    crypto.keccak256(concat(ADDR_REVERSE_NODE, label))
  );
}
import { ENSv2PrimaryName } from "./types/schema";
import { NameForAddrChanged } from "./types/DefaultReverseRegistrar/DefaultReverseRegistrar";

export function handleNameForAddrChanged(event: NameForAddrChanged): void {
  let account = createOrLoadAccount(event.params.addr);
  let id = event.params.addr;

  let primaryName = ENSv2PrimaryName.load(id);
  if (primaryName == null) {
    primaryName = new ENSv2PrimaryName(id);
    primaryName.account = account.id;
  }

  // Empty string is this contract's own clearing convention
  // (StandaloneReverseRegistrar._setName always emits unconditionally, with
  // no separate "name removed" event) — matches the clear-to-null pattern
  // used elsewhere in this schema.
  let name = event.params.name;
  primaryName.name = name.length > 0 ? name : null;
  primaryName.blockNumber = event.block.number;
  primaryName.transactionID = event.transaction.hash;
  primaryName.logIndex = event.logIndex;
  primaryName.reverseNode = legacyReverseNode(event.params.addr); // Blockscout/BENS extension
  primaryName.save();
}
