import { Address, BigInt, Bytes, crypto, ethereum } from "@graphprotocol/graph-ts";
import {
  assert,
  beforeAll,
  dataSourceMock,
  newMockEvent,
  test,
} from "matchstick-as/assembly/index";
import {
  handleNewOwner,
  handleNewOwnerOldRegistry,
  handleNewResolver,
  handleNewResolverOldRegistry,
  handleNewTTL,
  handleNewTTLOldRegistry,
  handleTransfer,
  handleTransferOldRegistry,
} from "../src/ensRegistry";
import {
  NewOwner,
  NewResolver,
  NewTTL,
  Transfer,
} from "../src/types/ENSRegistry/EnsRegistry";
import { Domain, WrappedDomain } from "../src/types/schema";
import { concat, createLegacyEventID, ROOT_NODE } from "../src/utils";

const ETH_NAMEHASH =
  "0x93cdeb708b7545dc668eb9280176169d1c33cfd8ed6f04690a0bcc88a93fc4ae";

const DEFAULT_OWNER = "0x89205A3A3b2A69De6Dbf7f01ED13B2108B2c43e7";

const DEFAULT_RESOLVER = "0x4976fb03C32e5B8cfe2b6cCB31c09Ba78EBaBa41";

const EMPTY_ADDRESS = "0x0000000000000000000000000000000000000000";

// Arbitrary, mutually distinct 32-byte node ids for the tests below --
// none of these need to be real namehashes, they just need to be stable
// and never collide with each other or with beforeAll's "eth" domain
// (ETH_NAMEHASH) or the domain the existing test above creates.
const NODE_TRANSFER =
  "0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const NODE_NEWTTL_EXISTS =
  "0xbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
const NODE_NEWTTL_MISSING =
  "0xcccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc";
const NODE_OLDOWNER_PARENT =
  "0xdddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd";
const NODE_OLDOWNER_LABEL =
  "0xeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee";
const NODE_RESOLVER_NONMIGRATED =
  "0x4444444444444444444444444444444444444444444444444444444444444444";
const NODE_RESOLVER_MIGRATED =
  "0x5555555555555555555555555555555555555555555555555555555555555555";
const NODE_TTL_NONMIGRATED =
  "0x6666666666666666666666666666666666666666666666666666666666666666";
const NODE_TTL_MIGRATED =
  "0x7777777777777777777777777777777777777777777777777777777777777777";
const NODE_XFER_NONMIGRATED =
  "0x8888888888888888888888888888888888888888888888888888888888888888";
const NODE_XFER_MIGRATED =
  "0x9999999999999999999999999999999999999999999999999999999999999999";

const createNewOwnerEvent = (
  node: string,
  label: string,
  owner: string
): NewOwner => {
  let mockEvent = newMockEvent();
  let newNewOwnerEvent = new NewOwner(
    mockEvent.address,
    mockEvent.logIndex,
    mockEvent.transactionLogIndex,
    mockEvent.logType,
    mockEvent.block,
    mockEvent.transaction,
    mockEvent.parameters,
    mockEvent.receipt
  );

  newNewOwnerEvent.parameters = new Array();
  let nodeParam = new ethereum.EventParam(
    "node",
    ethereum.Value.fromBytes(Bytes.fromHexString(node))
  );
  let labelParam = new ethereum.EventParam(
    "label",
    ethereum.Value.fromBytes(Bytes.fromHexString(label))
  );
  let ownerParam = new ethereum.EventParam(
    "owner",
    ethereum.Value.fromAddress(Address.fromString(owner))
  );
  newNewOwnerEvent.parameters.push(nodeParam);
  newNewOwnerEvent.parameters.push(labelParam);
  newNewOwnerEvent.parameters.push(ownerParam);
  return newNewOwnerEvent;
};

const createNewResolverEvent = (
  node: string,
  resolver: string
): NewResolver => {
  let mockEvent = newMockEvent();
  let newResolverEvent = new NewResolver(
    mockEvent.address,
    mockEvent.logIndex,
    mockEvent.transactionLogIndex,
    mockEvent.logType,
    mockEvent.block,
    mockEvent.transaction,
    mockEvent.parameters,
    mockEvent.receipt
  );

  newResolverEvent.parameters = new Array();
  let nodeParam = new ethereum.EventParam(
    "node",
    ethereum.Value.fromFixedBytes(Bytes.fromHexString(node))
  );
  let resolverParam = new ethereum.EventParam(
    "resolver",
    ethereum.Value.fromAddress(Address.fromString(resolver))
  );
  newResolverEvent.parameters.push(nodeParam);
  newResolverEvent.parameters.push(resolverParam);

  return newResolverEvent;
};

const createTransferEvent = (node: string, owner: string): Transfer => {
  let mockEvent = newMockEvent();
  let transferEvent = new Transfer(
    mockEvent.address,
    mockEvent.logIndex,
    mockEvent.transactionLogIndex,
    mockEvent.logType,
    mockEvent.block,
    mockEvent.transaction,
    mockEvent.parameters,
    mockEvent.receipt
  );

  transferEvent.parameters = new Array();
  let nodeParam = new ethereum.EventParam(
    "node",
    ethereum.Value.fromFixedBytes(Bytes.fromHexString(node))
  );
  let ownerParam = new ethereum.EventParam(
    "owner",
    ethereum.Value.fromAddress(Address.fromString(owner))
  );
  transferEvent.parameters.push(nodeParam);
  transferEvent.parameters.push(ownerParam);

  return transferEvent;
};

const createNewTTLEvent = (node: string, ttl: BigInt): NewTTL => {
  let mockEvent = newMockEvent();
  let newTTLEvent = new NewTTL(
    mockEvent.address,
    mockEvent.logIndex,
    mockEvent.transactionLogIndex,
    mockEvent.logType,
    mockEvent.block,
    mockEvent.transaction,
    mockEvent.parameters,
    mockEvent.receipt
  );

  newTTLEvent.parameters = new Array();
  let nodeParam = new ethereum.EventParam(
    "node",
    ethereum.Value.fromFixedBytes(Bytes.fromHexString(node))
  );
  let ttlParam = new ethereum.EventParam(
    "ttl",
    ethereum.Value.fromUnsignedBigInt(ttl)
  );
  newTTLEvent.parameters.push(nodeParam);
  newTTLEvent.parameters.push(ttlParam);

  return newTTLEvent;
};

// Directly constructs a minimally-valid Domain fixture (owner/isMigrated/
// subdomainCount/createdAt are all non-nullable in the schema) at an
// arbitrary node id, bypassing NewOwner entirely -- these tests are about
// handlers that require an existing Domain row, not about domain creation.
const seedDomain = (
  node: string,
  owner: string,
  isMigrated: boolean
): void => {
  let domain = new Domain(node);
  domain.storedOffchain = false; // Blockscout/BENS extension
  domain.resolvedWithWildcard = false; // Blockscout/BENS extension
  domain.owner = Address.fromString(owner).toHexString();
  domain.isMigrated = isMigrated;
  domain.subdomainCount = 0;
  domain.createdAt = BigInt.fromI32(0);
  domain.save();
};

beforeAll(() => {
  const ethLabelhash =
    "0x4f5b812789fc606be1b3b16908db13fc7a9adf7ca72641f84d75b47069d3d7f0";
  const emptyNode =
    "0x0000000000000000000000000000000000000000000000000000000000000000";
  const newNewOwnerEvent = createNewOwnerEvent(
    emptyNode,
    ethLabelhash,
    DEFAULT_OWNER
  );
  handleNewOwner(newNewOwnerEvent);
});

test("sets 0x0 resolver to null", () => {
  // something.eth
  const labelhash =
    "0x68371d7e884c168ae2022c82bd837d51837718a7f7dfb7aa3f753074a35e1d87";
  const namehash =
    "0x7857c9824139b8a8c3cb04712b41558b4878c55fa9c1e5390e910ee3220c3cce";
  const newNewOwnerEvent = createNewOwnerEvent(
    ETH_NAMEHASH,
    labelhash,
    DEFAULT_OWNER
  );
  handleNewOwner(newNewOwnerEvent);

  const newNewResolverEvent = createNewResolverEvent(
    namehash,
    DEFAULT_RESOLVER
  );
  handleNewResolver(newNewResolverEvent);

  let fetchedDomain = Domain.load(namehash)!;

  // assert.assertNotNull<T> does `value != null` internally, which crashes
  // the AS compiler for a nullable Bytes generic (a known compiler
  // gotcha) — use a truthy check assigned to a local boolean instead.
  let hasResolver = false;
  if (fetchedDomain.resolver) {
    hasResolver = true;
  }
  assert.assertTrue(hasResolver);

  const emptyResolverEvent = createNewResolverEvent(namehash, EMPTY_ADDRESS);
  handleNewResolver(emptyResolverEvent);

  fetchedDomain = Domain.load(namehash)!;

  // assert.assertNull<T> does `value == null` internally, which crashes the
  // AS compiler for a nullable Bytes generic (the same compiler
  // gotcha as assertNotNull above) — use a truthy check instead.
  assert.assertTrue(!fetchedDomain.resolver);
});

test("handleTransfer updates the domain owner and writes a Transfer history row", () => {
  seedDomain(NODE_TRANSFER, DEFAULT_OWNER, true);

  const newOwner = "0xF0205A3A3b2A69De6Dbf7f01ED13B2108B2c4321";
  const transferEvent = createTransferEvent(NODE_TRANSFER, newOwner);
  handleTransfer(transferEvent);

  assert.fieldEquals(
    "Domain",
    NODE_TRANSFER,
    "owner",
    Address.fromString(newOwner).toHexString()
  );

  let eventId = createLegacyEventID(transferEvent);
  assert.fieldEquals("Transfer", eventId, "domain", NODE_TRANSFER);
  assert.fieldEquals(
    "Transfer",
    eventId,
    "owner",
    Address.fromString(newOwner).toHexString()
  );
});

test("handleNewTTL sets ttl on an existing domain, and still writes history when the domain doesn't exist", () => {
  seedDomain(NODE_NEWTTL_EXISTS, DEFAULT_OWNER, true);

  const ttlEvent = createNewTTLEvent(NODE_NEWTTL_EXISTS, BigInt.fromI32(3600));
  handleNewTTL(ttlEvent);

  assert.fieldEquals("Domain", NODE_NEWTTL_EXISTS, "ttl", "3600");
  let eventId = createLegacyEventID(ttlEvent);
  assert.fieldEquals("NewTTL", eventId, "domain", NODE_NEWTTL_EXISTS);
  assert.fieldEquals("NewTTL", eventId, "ttl", "3600");

  // No domain exists at this node at all -- handleNewTTL's `if (domain)`
  // guard must skip the write without crashing, but the history row is
  // unconditional and still gets written.
  const missingDomainEvent = createNewTTLEvent(
    NODE_NEWTTL_MISSING,
    BigInt.fromI32(7200)
  );
  handleNewTTL(missingDomainEvent);

  assert.notInStore("Domain", NODE_NEWTTL_MISSING);
  let missingEventId = createLegacyEventID(missingDomainEvent);
  assert.fieldEquals("NewTTL", missingEventId, "domain", NODE_NEWTTL_MISSING);
  assert.fieldEquals("NewTTL", missingEventId, "ttl", "7200");
});

test("handleNewOwnerOldRegistry creates a fresh domain when none exists, then skips once the domain is migrated", () => {
  // Parent is ROOT_NODE, not a fabricated node: getDomain special-cases
  // ROOT_NODE to auto-synthesize a domain when none exists yet (matching
  // beforeAll's own "eth" setup above), whereas a made-up parent node would
  // have no Domain row at all and crash _handleNewOwner's `parent!`
  // assertion. subnode is a pure function of (node, label) via
  // keccak256(concat(node, label)) -- computed here the same way
  // src/ensRegistry.ts::makeSubnode does internally, so this test can
  // assert on/re-seed the exact row the handler itself will create, without
  // exporting that private helper.
  let subnode = Bytes.fromByteArray(
    crypto.keccak256(concat(ROOT_NODE, Bytes.fromHexString(NODE_OLDOWNER_LABEL)))
  );

  // No domain exists yet at subnode -> domain == null -> _handleNewOwner
  // runs with isMigrated: false.
  const firstEvent = createNewOwnerEvent(
    ROOT_NODE.toHexString(),
    NODE_OLDOWNER_LABEL,
    DEFAULT_OWNER
  );
  handleNewOwnerOldRegistry(firstEvent);

  assert.fieldEquals("Domain", subnode.toHexString(), "isMigrated", "false");
  assert.fieldEquals(
    "Domain",
    subnode.toHexString(),
    "owner",
    Address.fromString(DEFAULT_OWNER).toHexString()
  );

  // Domain already exists (from the first event above) AND is still
  // isMigrated: false -- a second, genuine repeated-update scenario on the
  // old registry before migration happens. Must still process (the gate is
  // `domain == null || domain.isMigrated == false`, not just `domain ==
  // null`), unlike the already-migrated case tested below.
  const secondOwner = "0xF0205A3A3b2A69De6Dbf7f01ED13B2108B2c4321";
  const secondEvent = createNewOwnerEvent(
    ROOT_NODE.toHexString(),
    NODE_OLDOWNER_LABEL,
    secondOwner
  );
  handleNewOwnerOldRegistry(secondEvent);

  assert.fieldEquals(
    "Domain",
    subnode.toHexString(),
    "owner",
    Address.fromString(secondOwner).toHexString()
  );
  assert.fieldEquals("Domain", subnode.toHexString(), "isMigrated", "false");

  // Migrate the domain (as the current ENSRegistry eventually would), then
  // fire the OldRegistry event again with a third owner -- must be a
  // complete no-op, since the old registry must never override state for a
  // domain that has already migrated to the current one.
  let domain = Domain.load(subnode.toHexString())!;
  domain.isMigrated = true;
  domain.save();

  const thirdOwner = "0x1234567890123456789012345678901234567890";
  const thirdEvent = createNewOwnerEvent(
    ROOT_NODE.toHexString(),
    NODE_OLDOWNER_LABEL,
    thirdOwner
  );
  handleNewOwnerOldRegistry(thirdEvent);

  assert.fieldEquals(
    "Domain",
    subnode.toHexString(),
    "owner",
    Address.fromString(secondOwner).toHexString()
  );
});

test("handleNewResolverOldRegistry processes for ROOT_NODE and a non-migrated domain, but skips an already-migrated domain", () => {
  // ROOT_NODE's own gate disjunct (node.equals(ROOT_NODE)) passes
  // unconditionally, regardless of getDomain's synthesized isMigrated: true
  // for the root.
  const rootResolverEvent = createNewResolverEvent(
    ROOT_NODE.toHexString(),
    DEFAULT_RESOLVER
  );
  handleNewResolverOldRegistry(rootResolverEvent);

  let rootDomain = Domain.load(ROOT_NODE.toHexString())!;
  let rootHasResolver = false;
  if (rootDomain.resolver) {
    rootHasResolver = true;
  }
  assert.assertTrue(rootHasResolver);

  // Non-root, non-migrated domain: delegates to handleNewResolver.
  seedDomain(NODE_RESOLVER_NONMIGRATED, DEFAULT_OWNER, false);
  const resolverEvent = createNewResolverEvent(
    NODE_RESOLVER_NONMIGRATED,
    DEFAULT_RESOLVER
  );
  handleNewResolverOldRegistry(resolverEvent);

  let domain = Domain.load(NODE_RESOLVER_NONMIGRATED)!;
  let hasResolver = false;
  if (domain.resolver) {
    hasResolver = true;
  }
  assert.assertTrue(hasResolver);

  // Already-migrated domain: must be left untouched.
  seedDomain(NODE_RESOLVER_MIGRATED, DEFAULT_OWNER, true);
  const migratedResolverEvent = createNewResolverEvent(
    NODE_RESOLVER_MIGRATED,
    DEFAULT_RESOLVER
  );
  handleNewResolverOldRegistry(migratedResolverEvent);

  let migratedDomain = Domain.load(NODE_RESOLVER_MIGRATED)!;
  let migratedHasResolver = false;
  if (migratedDomain.resolver) {
    migratedHasResolver = true;
  }
  assert.assertTrue(!migratedHasResolver);
});

test("handleNewTTLOldRegistry processes a non-migrated domain but skips an already-migrated one", () => {
  seedDomain(NODE_TTL_NONMIGRATED, DEFAULT_OWNER, false);
  const ttlEvent = createNewTTLEvent(
    NODE_TTL_NONMIGRATED,
    BigInt.fromI32(1800)
  );
  handleNewTTLOldRegistry(ttlEvent);

  assert.fieldEquals("Domain", NODE_TTL_NONMIGRATED, "ttl", "1800");

  seedDomain(NODE_TTL_MIGRATED, DEFAULT_OWNER, true);
  const migratedTtlEvent = createNewTTLEvent(
    NODE_TTL_MIGRATED,
    BigInt.fromI32(1800)
  );
  handleNewTTLOldRegistry(migratedTtlEvent);

  // ttl must stay unset (null) -- the old registry must not touch a domain
  // that has already migrated to the current one.
  let migratedDomain = Domain.load(NODE_TTL_MIGRATED)!;
  assert.assertTrue(!migratedDomain.ttl);
});

test("handleTransferOldRegistry processes a non-migrated domain but skips an already-migrated one", () => {
  const newOwner = "0xF0205A3A3b2A69De6Dbf7f01ED13B2108B2c4321";

  seedDomain(NODE_XFER_NONMIGRATED, DEFAULT_OWNER, false);
  const transferEvent = createTransferEvent(NODE_XFER_NONMIGRATED, newOwner);
  handleTransferOldRegistry(transferEvent);

  assert.fieldEquals(
    "Domain",
    NODE_XFER_NONMIGRATED,
    "owner",
    Address.fromString(newOwner).toHexString()
  );

  seedDomain(NODE_XFER_MIGRATED, DEFAULT_OWNER, true);
  const migratedTransferEvent = createTransferEvent(
    NODE_XFER_MIGRATED,
    newOwner
  );
  handleTransferOldRegistry(migratedTransferEvent);

  // owner must stay the original seeded value -- the old registry must not
  // override ownership for a domain that has already migrated.
  assert.fieldEquals(
    "Domain",
    NODE_XFER_MIGRATED,
    "owner",
    Address.fromString(DEFAULT_OWNER).toHexString()
  );
});

// --- wrapped names leaving the NameWrapper through the registry ---

const NAME_WRAPPER = "0xD4416b13d2b3a9aBae7AcD5D6C2BbDBE25686401";
const MAINNET_GRAVEYARD = "0xda021a2c1c2f99574d861ef8c6fa745851beb7f6";
const WRAPPED_HOLDER = "0x7c9e45243f0c67841ece2bfb160ae2721535a5e9";
const NODE_WRAPPED_TRANSFER =
  "0x1111111111111111111111111111111111111111111111111111111111111111";
const NODE_WRAPPED_GRAVEYARD =
  "0x2222222222222222222222222222222222222222222222222222222222222222";
const NODE_WRAPPED_NEWOWNER_LABEL =
  "0x3333333333333333333333333333333333333333333333333333333333333333";

function seedWrappedDomain(node: string): void {
  let domain = new Domain(node);
  domain.storedOffchain = false; // Blockscout/BENS extension
  domain.resolvedWithWildcard = false; // Blockscout/BENS extension
  domain.owner = Address.fromString(NAME_WRAPPER).toHexString();
  domain.wrappedOwner = Address.fromString(WRAPPED_HOLDER).toHexString();
  domain.isMigrated = true;
  domain.subdomainCount = 0;
  domain.createdAt = BigInt.fromI32(0);
  domain.save();

  let wrappedDomain = new WrappedDomain(node);
  wrappedDomain.domain = node;
  wrappedDomain.expiryDate = BigInt.fromI32(2000000000);
  wrappedDomain.fuses = 0;
  wrappedDomain.owner = Address.fromString(WRAPPED_HOLDER).toHexString();
  wrappedDomain.save();
}

test("handleTransfer moving a wrapped name away from the NameWrapper clears wrappedOwner and removes the WrappedDomain", () => {
  dataSourceMock.setNetwork("mainnet");
  seedWrappedDomain(NODE_WRAPPED_TRANSFER);

  const newOwner = "0xF0205A3A3b2A69De6Dbf7f01ED13B2108B2c4321";
  handleTransfer(createTransferEvent(NODE_WRAPPED_TRANSFER, newOwner));

  assert.fieldEquals(
    "Domain",
    NODE_WRAPPED_TRANSFER,
    "owner",
    Address.fromString(newOwner).toHexString()
  );
  assert.fieldEquals("Domain", NODE_WRAPPED_TRANSFER, "wrappedOwner", "null");
  assert.notInStore("WrappedDomain", NODE_WRAPPED_TRANSFER);
});

test("handleTransfer to the NameWrapper itself (wrap in progress) leaves wrapped state alone", () => {
  dataSourceMock.setNetwork("mainnet");
  seedWrappedDomain(NODE_WRAPPED_TRANSFER);

  handleTransfer(createTransferEvent(NODE_WRAPPED_TRANSFER, NAME_WRAPPER));

  assert.fieldEquals(
    "Domain",
    NODE_WRAPPED_TRANSFER,
    "wrappedOwner",
    Address.fromString(WRAPPED_HOLDER).toHexString()
  );
  assert.fieldEquals(
    "WrappedDomain",
    NODE_WRAPPED_TRANSFER,
    "owner",
    Address.fromString(WRAPPED_HOLDER).toHexString()
  );
});

test("handleTransfer to the ENSv2 Graveyard (migration voiding step) keeps the wrapped state for correctMigratedLegacyOwner", () => {
  dataSourceMock.setNetwork("mainnet");
  seedWrappedDomain(NODE_WRAPPED_GRAVEYARD);

  handleTransfer(createTransferEvent(NODE_WRAPPED_GRAVEYARD, MAINNET_GRAVEYARD));

  assert.fieldEquals("Domain", NODE_WRAPPED_GRAVEYARD, "owner", MAINNET_GRAVEYARD);
  assert.fieldEquals(
    "Domain",
    NODE_WRAPPED_GRAVEYARD,
    "wrappedOwner",
    Address.fromString(WRAPPED_HOLDER).toHexString()
  );
  assert.fieldEquals(
    "WrappedDomain",
    NODE_WRAPPED_GRAVEYARD,
    "owner",
    Address.fromString(WRAPPED_HOLDER).toHexString()
  );
});

test("handleNewOwner from the parent (setSubnodeOwner on a non-emancipated wrapped subname) clears the wrapped state", () => {
  dataSourceMock.setNetwork("mainnet");
  // Subname of ROOT_NODE so the parent Domain exists (see the old-registry
  // test above for why); the node is keccak256(parent ++ label).
  let subnode = Bytes.fromByteArray(
    crypto.keccak256(
      concat(ROOT_NODE, Bytes.fromHexString(NODE_WRAPPED_NEWOWNER_LABEL))
    )
  ).toHexString();
  seedWrappedDomain(subnode);

  const newOwner = "0xF0205A3A3b2A69De6Dbf7f01ED13B2108B2c4321";
  handleNewOwner(
    createNewOwnerEvent(
      ROOT_NODE.toHexString(),
      NODE_WRAPPED_NEWOWNER_LABEL,
      newOwner
    )
  );

  assert.fieldEquals(
    "Domain",
    subnode,
    "owner",
    Address.fromString(newOwner).toHexString()
  );
  assert.fieldEquals("Domain", subnode, "wrappedOwner", "null");
  assert.notInStore("WrappedDomain", subnode);
});
