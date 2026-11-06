import { Address, BigInt, Bytes, ethereum } from "@graphprotocol/graph-ts";
import {
  assert,
  beforeEach,
  clearStore,
  newMockEvent,
  test,
} from "matchstick-as/assembly/index";
import { handleAddrChanged, handleNameChanged } from "../src/resolver";
import {
  handleNameForAddrChanged,
  handleReverseClaimed,
} from "../src/reverseRecords";
import { AddrChanged, NameChanged } from "../src/types/Resolver/Resolver";
import { NameForAddrChanged } from "../src/types/DefaultReverseRegistrar/DefaultReverseRegistrar";
import { ReverseClaimed } from "../src/types/ReverseRegistrar/ReverseRegistrar";
import { Domain } from "../src/types/schema";

// Real mainnet fixture from the bug report: marketing.planetzephyros.etn had
// its reverse name set (block 15205162) before its addr record (15205738).
const WALLET = "0x4eb4b9ce208711a0ea1beff57c83bd66bc563378";
const OTHER_WALLET = "0x3fd2e5b4ac0eff6dfdf2446abddab3f66b425099";
const RESOLVER = "0xdb4a3abb6703232e20a118a104e7f4ebb3e2738d";

const NAME = "marketing.planetzephyros.etn";
// namehash(NAME)
const NAME_NODE =
  "0xaad0deb824cefdd9cf38aa3660d90499ee988bc26ff42eb73afc2c5ebb21876f";
// namehash("<WALLET>.addr.reverse")
const REVERSE_NODE =
  "0x5957a4641a1ef59b552444bb03b70f2b0ee4a007573effab30a71ec92b2d2c49";

const OTHER_NAME = "other.etn";
// namehash(OTHER_NAME)
const OTHER_NAME_NODE =
  "0xd647f5a752859354aabb1e8ec3f18a152bbe7c9fd547fca4331c6cf390b6c783";

const RECORD = "PrimaryNameRecord";
const CLAIM = "PrimaryNameClaim";

function resolverId(node: string): string {
  return RESOLVER.concat("-").concat(node);
}

function createDomain(node: string): void {
  let domain = new Domain(node);
  domain.resolver = resolverId(node);
  domain.subdomainCount = 0;
  domain.isMigrated = true;
  domain.createdAt = BigInt.fromI32(0);
  domain.owner = WALLET;
  domain.storedOffchain = false;
  domain.resolvedWithWildcard = false;
  domain.save();
}

function reverseClaimed(addr: string, node: string): ReverseClaimed {
  let mock = newMockEvent();
  let event = new ReverseClaimed(
    mock.address,
    mock.logIndex,
    mock.transactionLogIndex,
    mock.logType,
    mock.block,
    mock.transaction,
    mock.parameters,
    mock.receipt
  );
  event.parameters = new Array();
  event.parameters.push(
    new ethereum.EventParam(
      "addr",
      ethereum.Value.fromAddress(Address.fromString(addr))
    )
  );
  event.parameters.push(
    new ethereum.EventParam(
      "node",
      ethereum.Value.fromFixedBytes(Bytes.fromHexString(node))
    )
  );
  return event;
}

function nameChanged(node: string, name: string): NameChanged {
  let mock = newMockEvent();
  let event = new NameChanged(
    Address.fromString(RESOLVER),
    mock.logIndex,
    mock.transactionLogIndex,
    mock.logType,
    mock.block,
    mock.transaction,
    mock.parameters,
    mock.receipt
  );
  event.parameters = new Array();
  event.parameters.push(
    new ethereum.EventParam(
      "node",
      ethereum.Value.fromFixedBytes(Bytes.fromHexString(node))
    )
  );
  event.parameters.push(
    new ethereum.EventParam("name", ethereum.Value.fromString(name))
  );
  return event;
}

function addrChanged(node: string, a: string): AddrChanged {
  let mock = newMockEvent();
  let event = new AddrChanged(
    Address.fromString(RESOLVER),
    mock.logIndex,
    mock.transactionLogIndex,
    mock.logType,
    mock.block,
    mock.transaction,
    mock.parameters,
    mock.receipt
  );
  event.parameters = new Array();
  event.parameters.push(
    new ethereum.EventParam(
      "node",
      ethereum.Value.fromFixedBytes(Bytes.fromHexString(node))
    )
  );
  event.parameters.push(
    new ethereum.EventParam(
      "a",
      ethereum.Value.fromAddress(Address.fromString(a))
    )
  );
  return event;
}

function nameForAddrChanged(addr: string, name: string): NameForAddrChanged {
  let mock = newMockEvent();
  let event = new NameForAddrChanged(
    mock.address,
    mock.logIndex,
    mock.transactionLogIndex,
    mock.logType,
    mock.block,
    mock.transaction,
    mock.parameters,
    mock.receipt
  );
  event.parameters = new Array();
  event.parameters.push(
    new ethereum.EventParam(
      "addr",
      ethereum.Value.fromAddress(Address.fromString(addr))
    )
  );
  event.parameters.push(
    new ethereum.EventParam("name", ethereum.Value.fromString(name))
  );
  return event;
}

function setClassicName(name: string): void {
  handleReverseClaimed(reverseClaimed(WALLET, REVERSE_NODE));
  handleNameChanged(nameChanged(REVERSE_NODE, name));
}

beforeEach(() => {
  clearStore();
  createDomain(NAME_NODE);
  createDomain(OTHER_NAME_NODE);
});

test("classic reverse name set before addr record is recorded once addr catches up", () => {
  setClassicName(NAME);

  // Forward record not set yet: claim kept, record withheld.
  assert.fieldEquals(CLAIM, WALLET, "name", NAME);
  assert.notInStore(RECORD, WALLET);

  handleAddrChanged(addrChanged(NAME_NODE, WALLET));

  assert.fieldEquals(RECORD, WALLET, "domain_name", NAME);
  assert.fieldEquals(RECORD, WALLET, "domain_id", NAME_NODE);
  assert.fieldEquals(RECORD, WALLET, "resolved_address", WALLET);
});

test("record is dropped when the addr record moves away and restored when it comes back", () => {
  handleAddrChanged(addrChanged(NAME_NODE, WALLET));
  setClassicName(NAME);
  assert.fieldEquals(RECORD, WALLET, "domain_name", NAME);

  handleAddrChanged(addrChanged(NAME_NODE, OTHER_WALLET));
  assert.notInStore(RECORD, WALLET);
  // The other wallet never claimed the name, so it gets nothing either.
  assert.notInStore(RECORD, OTHER_WALLET);

  handleAddrChanged(addrChanged(NAME_NODE, WALLET));
  assert.fieldEquals(RECORD, WALLET, "domain_name", NAME);
});

test("empty classic name clears the record", () => {
  handleAddrChanged(addrChanged(NAME_NODE, WALLET));
  setClassicName(NAME);
  assert.fieldEquals(RECORD, WALLET, "domain_name", NAME);

  handleNameChanged(nameChanged(REVERSE_NODE, ""));
  assert.notInStore(RECORD, WALLET);
});

test("default reverse registrar name is used and re-verified on addr changes", () => {
  handleNameForAddrChanged(nameForAddrChanged(WALLET, NAME));
  assert.fieldEquals(CLAIM, WALLET, "defaultName", NAME);
  assert.notInStore(RECORD, WALLET);

  handleAddrChanged(addrChanged(NAME_NODE, WALLET));
  assert.fieldEquals(RECORD, WALLET, "domain_name", NAME);

  handleNameForAddrChanged(nameForAddrChanged(WALLET, ""));
  assert.notInStore(RECORD, WALLET);
});

test("classic name takes precedence over the default reverse registrar name", () => {
  handleAddrChanged(addrChanged(NAME_NODE, WALLET));
  handleAddrChanged(addrChanged(OTHER_NAME_NODE, WALLET));

  handleNameForAddrChanged(nameForAddrChanged(WALLET, OTHER_NAME));
  assert.fieldEquals(RECORD, WALLET, "domain_name", OTHER_NAME);

  setClassicName(NAME);
  assert.fieldEquals(RECORD, WALLET, "domain_name", NAME);

  // Clearing the classic name falls back to the default one.
  handleNameChanged(nameChanged(REVERSE_NODE, ""));
  assert.fieldEquals(RECORD, WALLET, "domain_name", OTHER_NAME);
});

test("a name that does not forward-resolve to the claimer is never recorded", () => {
  handleAddrChanged(addrChanged(NAME_NODE, OTHER_WALLET));
  setClassicName(NAME);
  assert.notInStore(RECORD, WALLET);
});
