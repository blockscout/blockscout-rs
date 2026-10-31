import { Address, BigInt, Bytes, ethereum } from "@graphprotocol/graph-ts";
import {
  assert,
  beforeEach,
  clearStore,
  describe,
  newMockEvent,
  test,
} from "matchstick-as/assembly/index";
import { handleNewOwner } from "../src/ensRegistry";
import { handleNameRegistered } from "../src/ethRegistrar";
import { handleNameWrapped } from "../src/nameWrapper";
import { NameRegistered } from "../src/types/BaseRegistrar/BaseRegistrar";
import { NameWrapped } from "../src/types/NameWrapper/NameWrapper";
import { ETH_NODE } from "../src/utils";
import { createNewOwnerEvent, setEthOwner } from "./testUtils";

// namehash("test.eth")
const TEST_ETH_NODE =
  "0xeb4f647bea6caa36333c816d7b46fdcb05f9466ecacc140ea8c66faf15b3d9f1";
// labelhash("test")
const TEST_LABELHASH =
  "0x9c22ff5f21f0b81b113e63f7db6da94fedef11b2119b4088b89664fb9a3cb658";
// uint256(labelhash("test")) == BaseRegistrar ERC721 token id
const TEST_TOKEN_ID =
  "70622639689279718371527342103894932928233838121221666359043189029713682937432";
// DNS-encoded "test.eth": [4]test[3]eth[0]
const TEST_DNS_NAME = "0x04746573740365746800";

// PARENT_CANNOT_CONTROL | CANNOT_UNWRAP -> a typical fully-emancipated .eth fuse set
const WRAPPED_FUSES = "196608";
const WRAPPED_EXPIRY = "1772118527";
const NEW_EXPIRY = "1813084307";

// Mirrors the live mainnet repro for theblackparade.eth:
// the stale wrappedOwner the buggy subgraph kept around...
const WRAPPED_OWNER_A = "0xb6accdb317341f8e20bf49ff2669c6c84e6c83c1";
// ...and the new registrant that actually owns the name on-chain.
const NEW_REGISTRANT_B = "0xf9e21edb4dfc9ff648da6ded3864b79e626e578a";
// The NameWrapper holds the BaseRegistrar NFT during a register-and-wrap, so it
// is the `owner` reported by the BaseRegistrar NameRegistered event in that flow.
const NAME_WRAPPER_ADDRESS = "0xd4416b13d2b3a9abae7acd5d6c2bbdbe25686401";

const createNameWrappedEvent = (
  node: string,
  name: string,
  owner: string,
  fuses: string,
  expiry: string
): NameWrapped => {
  let mockEvent = newMockEvent();
  let nameWrappedEvent = new NameWrapped(
    mockEvent.address,
    mockEvent.logIndex,
    mockEvent.transactionLogIndex,
    mockEvent.logType,
    mockEvent.block,
    mockEvent.transaction,
    mockEvent.parameters,
    mockEvent.receipt
  );
  nameWrappedEvent.parameters = new Array();
  nameWrappedEvent.parameters.push(
    new ethereum.EventParam(
      "node",
      ethereum.Value.fromBytes(Bytes.fromHexString(node))
    )
  );
  nameWrappedEvent.parameters.push(
    new ethereum.EventParam(
      "name",
      ethereum.Value.fromBytes(Bytes.fromHexString(name))
    )
  );
  nameWrappedEvent.parameters.push(
    new ethereum.EventParam(
      "owner",
      ethereum.Value.fromAddress(Address.fromString(owner))
    )
  );
  nameWrappedEvent.parameters.push(
    new ethereum.EventParam(
      "fuses",
      ethereum.Value.fromUnsignedBigInt(BigInt.fromString(fuses))
    )
  );
  nameWrappedEvent.parameters.push(
    new ethereum.EventParam(
      "expiry",
      ethereum.Value.fromUnsignedBigInt(BigInt.fromString(expiry))
    )
  );
  return nameWrappedEvent;
};

const createNameRegisteredEvent = (
  id: string,
  owner: string,
  expires: string
): NameRegistered => {
  let mockEvent = newMockEvent();
  let nameRegisteredEvent = new NameRegistered(
    mockEvent.address,
    mockEvent.logIndex,
    mockEvent.transactionLogIndex,
    mockEvent.logType,
    mockEvent.block,
    mockEvent.transaction,
    mockEvent.parameters,
    mockEvent.receipt
  );
  nameRegisteredEvent.parameters = new Array();
  nameRegisteredEvent.parameters.push(
    new ethereum.EventParam(
      "id",
      ethereum.Value.fromUnsignedBigInt(BigInt.fromString(id))
    )
  );
  nameRegisteredEvent.parameters.push(
    new ethereum.EventParam(
      "owner",
      ethereum.Value.fromAddress(Address.fromString(owner))
    )
  );
  nameRegisteredEvent.parameters.push(
    new ethereum.EventParam(
      "expires",
      ethereum.Value.fromUnsignedBigInt(BigInt.fromString(expires))
    )
  );
  return nameRegisteredEvent;
};

const createTestEthDomain = (owner: string): void => {
  handleNewOwner(createNewOwnerEvent(ETH_NODE.toHexString(), TEST_LABELHASH, owner));
};

describe("wrapped name re-registration", () => {
  beforeEach(() => {
    clearStore();
    setEthOwner();
  });

  test("clears stale wrapped state when an expired wrapped name is re-registered", () => {
    createTestEthDomain(WRAPPED_OWNER_A);

    // Name is wrapped under owner A.
    handleNameWrapped(
      createNameWrappedEvent(
        TEST_ETH_NODE,
        TEST_DNS_NAME,
        WRAPPED_OWNER_A,
        WRAPPED_FUSES,
        WRAPPED_EXPIRY
      )
    );

    // Sanity check: the wrapped state exists before re-registration.
    assert.fieldEquals("Domain", TEST_ETH_NODE, "wrappedOwner", WRAPPED_OWNER_A);
    assert.fieldEquals("WrappedDomain", TEST_ETH_NODE, "owner", WRAPPED_OWNER_A);

    // The name expires (NameWrapper emits NO event for expiry) and is later
    // re-registered by a different owner via the BaseRegistrar.
    handleNameRegistered(
      createNameRegisteredEvent(TEST_TOKEN_ID, NEW_REGISTRANT_B, NEW_EXPIRY)
    );

    // The stale wrapped state must be gone...
    assert.fieldEquals("Domain", TEST_ETH_NODE, "wrappedOwner", "null");
    assert.notInStore("WrappedDomain", TEST_ETH_NODE);
    // ...and ownership reflects the new registrant.
    assert.fieldEquals(
      "Domain",
      TEST_ETH_NODE,
      "registrant",
      NEW_REGISTRANT_B
    );
  });

  test("register-and-wrap after a lapsed wrapped registration keeps the new wrapped owner", () => {
    createTestEthDomain(WRAPPED_OWNER_A);

    // Prior life: the name is wrapped under owner A.
    handleNameWrapped(
      createNameWrappedEvent(
        TEST_ETH_NODE,
        TEST_DNS_NAME,
        WRAPPED_OWNER_A,
        WRAPPED_FUSES,
        WRAPPED_EXPIRY
      )
    );
    assert.fieldEquals("WrappedDomain", TEST_ETH_NODE, "owner", WRAPPED_OWNER_A);

    // The name expires (no event) and is re-registered AND re-wrapped in the same
    // tx by a new owner. On-chain log order within that tx is:
    //   BaseRegistrar.NameRegistered (owner = NameWrapper) -> NameWrapper.NameWrapped
    // handleNameRegistered clears the stale owner-A wrapped state, then
    // handleNameWrapped recreates the WrappedDomain under the new owner. This guards
    // the register-and-wrap path: the clear must not leave the name unwrapped.
    handleNameRegistered(
      createNameRegisteredEvent(TEST_TOKEN_ID, NAME_WRAPPER_ADDRESS, NEW_EXPIRY)
    );
    handleNameWrapped(
      createNameWrappedEvent(
        TEST_ETH_NODE,
        TEST_DNS_NAME,
        NEW_REGISTRANT_B,
        WRAPPED_FUSES,
        NEW_EXPIRY
      )
    );

    // Final state must be wrapped under the new owner — never stuck on null (the
    // cleared value) and never the stale owner A.
    assert.fieldEquals("WrappedDomain", TEST_ETH_NODE, "owner", NEW_REGISTRANT_B);
    assert.fieldEquals("Domain", TEST_ETH_NODE, "wrappedOwner", NEW_REGISTRANT_B);
  });
});
