import { Address, BigInt, Bytes, crypto, ethereum } from "@graphprotocol/graph-ts";
import {
  assert,
  beforeAll,
  describe,
  newMockEvent,
  test,
} from "matchstick-as/assembly/index";
import { handleNewOwner } from "../src/ensRegistry";
import {
  handleNameRegistered,
  handleNameRegisteredByLegacyController,
  handleNameRegisteredByUnwrappedController,
  handleNameRegisteredByWrappedController,
  handleNameRenewed,
  handleNameRenewedByLegacyController,
  handleNameRenewedByUnwrappedController,
  handleNameTransferred,
} from "../src/ethRegistrar";
import {
  NameRegistered,
  NameRenewed as BaseRegistrarNameRenewed,
  Transfer as BaseRegistrarTransfer,
} from "../src/types/BaseRegistrar/BaseRegistrar";
import {
  NameRegistered as LegacyNameRegistered,
  NameRenewed as LegacyNameRenewed,
} from "../src/types/LegacyEthRegistrarController/LegacyEthRegistrarController";
import { Domain, Registration ,
  WrappedDomain,
} from "../src/types/schema";
import {
  NameRegistered as UnwrappedEthRegistrarController_NameRegistered,
  NameRenewed as UnwrappedEthRegistrarController_NameRenewed,
} from "../src/types/UnwrappedEthRegistrarController/UnwrappedEthRegistrarController";
import { NameRegistered as WrappedEthRegistrarController_NameRegistered } from "../src/types/WrappedEthRegistrarController/WrappedEthRegistrarController";
import { concat, createLegacyEventID, ETH_NODE } from "../src/utils";
import { createNewOwnerEvent, DEFAULT_OWNER, setEthOwner } from "./testUtils";

describe("legacy/wrapped controller", () => {
  const createWrappedEthRegistrarController_NameRegistered = (
    name: string,
    label: string,
    owner: string,
    expires: string
  ): WrappedEthRegistrarController_NameRegistered => {
    let mockEvent = newMockEvent();
    let nameRegisteredByControllerEvent =
      new WrappedEthRegistrarController_NameRegistered(
        mockEvent.address,
        mockEvent.logIndex,
        mockEvent.transactionLogIndex,
        mockEvent.logType,
        mockEvent.block,
        mockEvent.transaction,
        mockEvent.parameters,
        mockEvent.receipt
      );

    nameRegisteredByControllerEvent.parameters = new Array();
    let nameParam = new ethereum.EventParam(
      "name",
      ethereum.Value.fromString(name)
    );
    let labelParam = new ethereum.EventParam(
      "label",
      ethereum.Value.fromBytes(Bytes.fromHexString(label))
    );
    let ownerParam = new ethereum.EventParam(
      "owner",
      ethereum.Value.fromAddress(Address.fromString(owner))
    );
    let baseCostParam = new ethereum.EventParam(
      "baseCost",
      ethereum.Value.fromSignedBigInt(BigInt.fromI32(0))
    );
    let premiumParam = new ethereum.EventParam(
      "premium",
      ethereum.Value.fromSignedBigInt(BigInt.fromI32(0))
    );
    let expiresParam = new ethereum.EventParam(
      "expires",
      ethereum.Value.fromSignedBigInt(BigInt.fromString(expires))
    );
    nameRegisteredByControllerEvent.parameters.push(nameParam);
    nameRegisteredByControllerEvent.parameters.push(labelParam);
    nameRegisteredByControllerEvent.parameters.push(ownerParam);
    nameRegisteredByControllerEvent.parameters.push(baseCostParam);
    nameRegisteredByControllerEvent.parameters.push(premiumParam);
    nameRegisteredByControllerEvent.parameters.push(expiresParam);

    return nameRegisteredByControllerEvent;
  };

  const createNameRegisteredEvent = (
    id: string,
    owner: string,
    expires: string
  ): NameRegistered => {
    let mockEvent = newMockEvent();
    let newNameRegisteredEvent = new NameRegistered(
      mockEvent.address,
      mockEvent.logIndex,
      mockEvent.transactionLogIndex,
      mockEvent.logType,
      mockEvent.block,
      mockEvent.transaction,
      mockEvent.parameters,
      mockEvent.receipt
    );
    newNameRegisteredEvent.parameters = new Array();
    let idParam = new ethereum.EventParam(
      "id",
      ethereum.Value.fromSignedBigInt(BigInt.fromString(id))
    );
    let ownerParam = new ethereum.EventParam(
      "owner",
      ethereum.Value.fromAddress(Address.fromString(owner))
    );
    let expiresParam = new ethereum.EventParam(
      "expires",
      ethereum.Value.fromSignedBigInt(BigInt.fromString(expires))
    );
    newNameRegisteredEvent.parameters.push(idParam);
    newNameRegisteredEvent.parameters.push(ownerParam);
    newNameRegisteredEvent.parameters.push(expiresParam);
    return newNameRegisteredEvent;
  };

  beforeAll(() => {
    setEthOwner();
  });

  const checkNullLabelName = (
    labelhash: string,
    labelhashAsInt: string,
    label: string
  ): void => {
    const newNewOwnerEvent = createNewOwnerEvent(
      ETH_NODE.toHexString(),
      labelhash,
      DEFAULT_OWNER
    );
    handleNewOwner(newNewOwnerEvent);

    let newRegistrationEvent = createNameRegisteredEvent(
      labelhashAsInt,
      DEFAULT_OWNER,
      "1610000000"
    );
    handleNameRegistered(newRegistrationEvent);

    let fetchedRegistration = Registration.load(labelhash)!;

    // set labelName to null because handleNameRegistered sets it to a mocked value of "default"
    // which comes from ens.nameByHash()
    fetchedRegistration.labelName = null;
    fetchedRegistration.save();

    const nameRegisteredByControllerEvent =
      createWrappedEthRegistrarController_NameRegistered(
        label,
        labelhash,
        DEFAULT_OWNER,
        "1610000000"
      );
    handleNameRegisteredByWrappedController(nameRegisteredByControllerEvent);

    fetchedRegistration = Registration.load(labelhash)!;

    assert.assertNull(fetchedRegistration.labelName);
  };

  test("does not assign label name to null byte label", () => {
    const labelhash =
      "0x465b93df44674596a1f5cd92ec83053bb8a78f6083e1752b3162c739bba1f9ed";
    const labelhashAsInt =
      "31823703059708284547668674100687316300171847632515296374731848165239501748717";
    const label = "default\0";

    checkNullLabelName(labelhash, labelhashAsInt, label);
  });

  test("does not assign label name to label with '.' separator", () => {
    const labelhash =
      "0xf8a2e15376341ae37c90b754e5ef3f1e43d1d136a5c7ba6b34c50b466848dfbc";
    const labelhashAsInt =
      "112461370816196049012812662280597321405198137204162513382374556989424524648380";
    const label = "test.123";

    checkNullLabelName(labelhash, labelhashAsInt, label);
  });

  test("does not assign label name to label with '[' char", () => {
    const labelhash =
      "0x6d2df8d29c51e5e79bce0067df6a093fd7e535f1fe0a509ead1eb5a2171640c9";
    const labelhashAsInt =
      "49383325924636276199200854251362239534766035480602437112552046254651845525705";
    const label =
      "[41ff1915eef2bf5841388d748bfcd23bd49ff5521ca4200c20bc0978b136c3cb";

    checkNullLabelName(labelhash, labelhashAsInt, label);
  });

  test("does not assign label name to label with ']' char", () => {
    const labelhash =
      "0xb9cf267ed9b0cb8caf44655901be5b66f2e6bbedd8dc1436fba973f7a824db58";
    const labelhashAsInt =
      "84043880016553362091807057514212448616446700818045523307434280128309910362968";
    const label =
      "41ff1915eef2bf5841388d748bfcd23bd49ff5521ca4200c20bc0978b136c3cb]";

    checkNullLabelName(labelhash, labelhashAsInt, label);
  });

  test("does not assign label name to label that uses unnormalised label notation", () => {
    const labelhash =
      "0x162894963b59f9b7e47a34709830c0211a6ba5f7de3973839f3ee7002e0c8434";
    const labelhashAsInt =
      "10022582060124759960163130513734713560279061696053801337665848910969813369908";
    const label =
      "[41ff1915eef2bf5841388d748bfcd23bd49ff5521ca4200c20bc0978b136c3cb]";

    checkNullLabelName(labelhash, labelhashAsInt, label);
  });

  test("does assign normal label", () => {
    const labelhash =
      "0x9c22ff5f21f0b81b113e63f7db6da94fedef11b2119b4088b89664fb9a3cb658";
    const labelhashAsInt =
      "70622639689279718371527342103894932928233838121221666359043189029713682937432";
    const label = "test";

    const newNewOwnerEvent = createNewOwnerEvent(
      ETH_NODE.toHexString(),
      labelhash,
      DEFAULT_OWNER
    );
    handleNewOwner(newNewOwnerEvent);

    let newRegistrationEvent = createNameRegisteredEvent(
      labelhashAsInt,
      DEFAULT_OWNER,
      "1610000000"
    );
    handleNameRegistered(newRegistrationEvent);

    let fetchedRegistration = Registration.load(labelhash)!;

    fetchedRegistration.labelName = "eth";
    fetchedRegistration.save();

    const nameRegisteredByControllerEvent =
      createWrappedEthRegistrarController_NameRegistered(
        label,
        labelhash,
        DEFAULT_OWNER,
        "1610000000"
      );
    handleNameRegisteredByWrappedController(nameRegisteredByControllerEvent);

    fetchedRegistration = Registration.load(labelhash)!;

    assert.assertTrue(fetchedRegistration.labelName == label);
  });
});

describe("unwrapped controller", () => {
  const createUnwrappedEthRegistrarController_NameRegistered = (
    label: string,
    labelhash: string,
    owner: string,
    baseCost: string,
    premium: string,
    expires: string,
    referrer: string = "0x0000000000000000000000000000000000000000000000000000000000000000"
  ): UnwrappedEthRegistrarController_NameRegistered => {
    let mockEvent = newMockEvent();
    let nameRegisteredEvent =
      new UnwrappedEthRegistrarController_NameRegistered(
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
    let labelParam = new ethereum.EventParam(
      "label",
      ethereum.Value.fromString(label)
    );
    let labelhashParam = new ethereum.EventParam(
      "labelhash",
      ethereum.Value.fromBytes(Bytes.fromHexString(labelhash))
    );
    let ownerParam = new ethereum.EventParam(
      "owner",
      ethereum.Value.fromAddress(Address.fromString(owner))
    );
    let baseCostParam = new ethereum.EventParam(
      "baseCost",
      ethereum.Value.fromSignedBigInt(BigInt.fromString(baseCost))
    );
    let premiumParam = new ethereum.EventParam(
      "premium",
      ethereum.Value.fromSignedBigInt(BigInt.fromString(premium))
    );
    let expiresParam = new ethereum.EventParam(
      "expires",
      ethereum.Value.fromSignedBigInt(BigInt.fromString(expires))
    );
    let referrerParam = new ethereum.EventParam(
      "referrer",
      ethereum.Value.fromBytes(Bytes.fromHexString(referrer))
    );

    nameRegisteredEvent.parameters.push(labelParam);
    nameRegisteredEvent.parameters.push(labelhashParam);
    nameRegisteredEvent.parameters.push(ownerParam);
    nameRegisteredEvent.parameters.push(baseCostParam);
    nameRegisteredEvent.parameters.push(premiumParam);
    nameRegisteredEvent.parameters.push(expiresParam);
    nameRegisteredEvent.parameters.push(referrerParam);

    return nameRegisteredEvent;
  };

  const createUnwrappedEthRegistrarController_NameRenewed = (
    label: string,
    labelhash: string,
    cost: string,
    expires: string,
    referrer: string = "0x0000000000000000000000000000000000000000000000000000000000000000"
  ): UnwrappedEthRegistrarController_NameRenewed => {
    let mockEvent = newMockEvent();
    let nameRenewedEvent = new UnwrappedEthRegistrarController_NameRenewed(
      mockEvent.address,
      mockEvent.logIndex,
      mockEvent.transactionLogIndex,
      mockEvent.logType,
      mockEvent.block,
      mockEvent.transaction,
      mockEvent.parameters,
      mockEvent.receipt
    );

    nameRenewedEvent.parameters = new Array();
    let labelParam = new ethereum.EventParam(
      "label",
      ethereum.Value.fromString(label)
    );
    let labelhashParam = new ethereum.EventParam(
      "labelhash",
      ethereum.Value.fromBytes(Bytes.fromHexString(labelhash))
    );
    let costParam = new ethereum.EventParam(
      "cost",
      ethereum.Value.fromSignedBigInt(BigInt.fromString(cost))
    );
    let expiresParam = new ethereum.EventParam(
      "expires",
      ethereum.Value.fromSignedBigInt(BigInt.fromString(expires))
    );
    let referrerParam = new ethereum.EventParam(
      "referrer",
      ethereum.Value.fromBytes(Bytes.fromHexString(referrer))
    );

    nameRenewedEvent.parameters.push(labelParam);
    nameRenewedEvent.parameters.push(labelhashParam);
    nameRenewedEvent.parameters.push(costParam);
    nameRenewedEvent.parameters.push(expiresParam);
    nameRenewedEvent.parameters.push(referrerParam);

    return nameRenewedEvent;
  };

  const createNameRegisteredEvent = (
    id: string,
    owner: string,
    expires: string
  ): NameRegistered => {
    let mockEvent = newMockEvent();
    let newNameRegisteredEvent = new NameRegistered(
      mockEvent.address,
      mockEvent.logIndex,
      mockEvent.transactionLogIndex,
      mockEvent.logType,
      mockEvent.block,
      mockEvent.transaction,
      mockEvent.parameters,
      mockEvent.receipt
    );
    newNameRegisteredEvent.parameters = new Array();
    let idParam = new ethereum.EventParam(
      "id",
      ethereum.Value.fromSignedBigInt(BigInt.fromString(id))
    );
    let ownerParam = new ethereum.EventParam(
      "owner",
      ethereum.Value.fromAddress(Address.fromString(owner))
    );
    let expiresParam = new ethereum.EventParam(
      "expires",
      ethereum.Value.fromSignedBigInt(BigInt.fromString(expires))
    );
    newNameRegisteredEvent.parameters.push(idParam);
    newNameRegisteredEvent.parameters.push(ownerParam);
    newNameRegisteredEvent.parameters.push(expiresParam);
    return newNameRegisteredEvent;
  };

  beforeAll(() => {
    setEthOwner();
  });

  const checkNullLabelNameForUnwrapped = (
    labelhash: string,
    labelhashAsInt: string,
    label: string
  ): void => {
    const newNewOwnerEvent = createNewOwnerEvent(
      ETH_NODE.toHexString(),
      labelhash,
      DEFAULT_OWNER
    );
    handleNewOwner(newNewOwnerEvent);

    let newRegistrationEvent = createNameRegisteredEvent(
      labelhashAsInt,
      DEFAULT_OWNER,
      "1610000000"
    );
    handleNameRegistered(newRegistrationEvent);

    let fetchedRegistration = Registration.load(labelhash)!;

    // set labelName to null because handleNameRegistered sets it to a mocked value of "default"
    // which comes from ens.nameByHash()
    fetchedRegistration.labelName = null;
    fetchedRegistration.save();

    const nameRegisteredByUnwrappedControllerEvent =
      createUnwrappedEthRegistrarController_NameRegistered(
        label,
        labelhash,
        DEFAULT_OWNER,
        "1000000000",
        "0",
        "1610000000"
      );
    handleNameRegisteredByUnwrappedController(
      nameRegisteredByUnwrappedControllerEvent
    );

    fetchedRegistration = Registration.load(labelhash)!;

    assert.assertNull(fetchedRegistration.labelName);
  };

  test("does not assign label name to null byte label", () => {
    const labelhash =
      "0x465b93df44674596a1f5cd92ec83053bb8a78f6083e1752b3162c739bba1f9ed";
    const labelhashAsInt =
      "31823703059708284547668674100687316300171847632515296374731848165239501748717";
    const label = "default\0";

    checkNullLabelNameForUnwrapped(labelhash, labelhashAsInt, label);
  });

  test("does not assign label name to label with '.' separator", () => {
    const labelhash =
      "0xf8a2e15376341ae37c90b754e5ef3f1e43d1d136a5c7ba6b34c50b466848dfbc";
    const labelhashAsInt =
      "112461370816196049012812662280597321405198137204162513382374556989424524648380";
    const label = "test.123";

    checkNullLabelNameForUnwrapped(labelhash, labelhashAsInt, label);
  });

  test("does not assign label name to label with '[' char", () => {
    const labelhash =
      "0x6d2df8d29c51e5e79bce0067df6a093fd7e535f1fe0a509ead1eb5a2171640c9";
    const labelhashAsInt =
      "49383325924636276199200854251362239534766035480602437112552046254651845525705";
    const label =
      "[41ff1915eef2bf5841388d748bfcd23bd49ff5521ca4200c20bc0978b136c3cb";

    checkNullLabelNameForUnwrapped(labelhash, labelhashAsInt, label);
  });

  test("does not assign label name to label with ']' char", () => {
    const labelhash =
      "0xb9cf267ed9b0cb8caf44655901be5b66f2e6bbedd8dc1436fba973f7a824db58";
    const labelhashAsInt =
      "84043880016553362091807057514212448616446700818045523307434280128309910362968";
    const label =
      "41ff1915eef2bf5841388d748bfcd23bd49ff5521ca4200c20bc0978b136c3cb]";

    checkNullLabelNameForUnwrapped(labelhash, labelhashAsInt, label);
  });

  test("does not assign label name to label that uses unnormalised label notation", () => {
    const labelhash =
      "0x162894963b59f9b7e47a34709830c0211a6ba5f7de3973839f3ee7002e0c8434";
    const labelhashAsInt =
      "10022582060124759960163130513734713560279061696053801337665848910969813369908";
    const label =
      "[41ff1915eef2bf5841388d748bfcd23bd49ff5521ca4200c20bc0978b136c3cb]";

    checkNullLabelNameForUnwrapped(labelhash, labelhashAsInt, label);
  });

  test("does assign normal label", () => {
    const labelhash =
      "0x9c22ff5f21f0b81b113e63f7db6da94fedef11b2119b4088b89664fb9a3cb658";
    const labelhashAsInt =
      "70622639689279718371527342103894932928233838121221666359043189029713682937432";
    const label = "test";

    const newNewOwnerEvent = createNewOwnerEvent(
      ETH_NODE.toHexString(),
      labelhash,
      DEFAULT_OWNER
    );
    handleNewOwner(newNewOwnerEvent);

    let newRegistrationEvent = createNameRegisteredEvent(
      labelhashAsInt,
      DEFAULT_OWNER,
      "1610000000"
    );
    handleNameRegistered(newRegistrationEvent);

    let fetchedRegistration = Registration.load(labelhash)!;

    fetchedRegistration.labelName = "eth";
    fetchedRegistration.save();

    const nameRegisteredByUnwrappedControllerEvent =
      createUnwrappedEthRegistrarController_NameRegistered(
        label,
        labelhash,
        DEFAULT_OWNER,
        "1000000000",
        "500000000",
        "1610000000"
      );
    handleNameRegisteredByUnwrappedController(
      nameRegisteredByUnwrappedControllerEvent
    );

    fetchedRegistration = Registration.load(labelhash)!;

    assert.assertTrue(fetchedRegistration.labelName == label);
    assert.assertTrue(
      fetchedRegistration.cost!.equals(BigInt.fromString("1500000000"))
    );
  });

  test("handles renewal correctly", () => {
    const labelhash =
      "0x9c22ff5f21f0b81b113e63f7db6da94fedef11b2119b4088b89664fb9a3cb658";
    const labelhashAsInt =
      "70622639689279718371527342103894932928233838121221666359043189029713682937432";
    const label = "test";

    // First register the name
    const newNewOwnerEvent = createNewOwnerEvent(
      ETH_NODE.toHexString(),
      labelhash,
      DEFAULT_OWNER
    );
    handleNewOwner(newNewOwnerEvent);

    let newRegistrationEvent = createNameRegisteredEvent(
      labelhashAsInt,
      DEFAULT_OWNER,
      "1610000000"
    );
    handleNameRegistered(newRegistrationEvent);

    let fetchedRegistration = Registration.load(labelhash)!;

    fetchedRegistration.labelName = "eth";
    fetchedRegistration.save();

    // Register via unwrapped controller first
    const nameRegisteredByUnwrappedControllerEvent =
      createUnwrappedEthRegistrarController_NameRegistered(
        label,
        labelhash,
        DEFAULT_OWNER,
        "1000000000",
        "0",
        "1610000000"
      );
    handleNameRegisteredByUnwrappedController(
      nameRegisteredByUnwrappedControllerEvent
    );

    fetchedRegistration = Registration.load(labelhash)!;
    assert.assertTrue(fetchedRegistration.labelName == label);
    assert.assertTrue(
      fetchedRegistration.cost!.equals(BigInt.fromString("1000000000"))
    );

    // Now renew it
    const nameRenewedByUnwrappedControllerEvent =
      createUnwrappedEthRegistrarController_NameRenewed(
        label,
        labelhash,
        "2000000000",
        "1620000000"
      );
    handleNameRenewedByUnwrappedController(
      nameRenewedByUnwrappedControllerEvent
    );

    fetchedRegistration = Registration.load(labelhash)!;
    assert.assertTrue(fetchedRegistration.labelName == label);
    assert.assertTrue(
      fetchedRegistration.cost!.equals(BigInt.fromString("2000000000"))
    );
  });

  test("does not process renewal for invalid label", () => {
    const labelhash =
      "0x5565c8492bfb46737a4c15de29b8ad5011d2510ddf5bc46dc28a1d622beb16a6";
    const labelhashAsInt =
      "38626426005097521807023039862862454575472987906264168594156980995768502130342";
    const invalidLabel = "invalid.label";

    // First register the name
    const newNewOwnerEvent = createNewOwnerEvent(
      ETH_NODE.toHexString(),
      labelhash,
      DEFAULT_OWNER
    );
    handleNewOwner(newNewOwnerEvent);

    let newRegistrationEvent = createNameRegisteredEvent(
      labelhashAsInt,
      DEFAULT_OWNER,
      "1610000000"
    );
    handleNameRegistered(newRegistrationEvent);

    let fetchedRegistration = Registration.load(labelhash)!;
    fetchedRegistration.labelName = null;
    fetchedRegistration.cost = BigInt.fromString("1000000000");
    fetchedRegistration.save();

    // Renew
    const nameRenewedByUnwrappedControllerEvent =
      createUnwrappedEthRegistrarController_NameRenewed(
        invalidLabel,
        labelhash,
        "2000000000",
        "1620000000"
      );
    handleNameRenewedByUnwrappedController(
      nameRenewedByUnwrappedControllerEvent
    );

    fetchedRegistration = Registration.load(labelhash)!;
    assert.assertTrue(
      fetchedRegistration.cost!.equals(BigInt.fromString("1000000000"))
    );
    assert.assertNull(fetchedRegistration.labelName);
  });
});

describe("BaseRegistrar's own NameRenewed/Transfer, and LegacyEthRegistrarController", () => {
  const createNameRenewedEvent = (id: string, expires: string): BaseRegistrarNameRenewed => {
    let mockEvent = newMockEvent();
    let event = new BaseRegistrarNameRenewed(
      mockEvent.address,
      mockEvent.logIndex,
      mockEvent.transactionLogIndex,
      mockEvent.logType,
      mockEvent.block,
      mockEvent.transaction,
      mockEvent.parameters,
      mockEvent.receipt
    );
    event.parameters = new Array();
    event.parameters.push(
      new ethereum.EventParam("id", ethereum.Value.fromSignedBigInt(BigInt.fromString(id)))
    );
    event.parameters.push(
      new ethereum.EventParam("expires", ethereum.Value.fromSignedBigInt(BigInt.fromString(expires)))
    );
    return event;
  };

  const createTransferEvent = (
    from: string,
    to: string,
    tokenId: string
  ): BaseRegistrarTransfer => {
    let mockEvent = newMockEvent();
    let event = new BaseRegistrarTransfer(
      mockEvent.address,
      mockEvent.logIndex,
      mockEvent.transactionLogIndex,
      mockEvent.logType,
      mockEvent.block,
      mockEvent.transaction,
      mockEvent.parameters,
      mockEvent.receipt
    );
    event.parameters = new Array();
    event.parameters.push(
      new ethereum.EventParam("from", ethereum.Value.fromAddress(Address.fromString(from)))
    );
    event.parameters.push(
      new ethereum.EventParam("to", ethereum.Value.fromAddress(Address.fromString(to)))
    );
    event.parameters.push(
      new ethereum.EventParam("tokenId", ethereum.Value.fromSignedBigInt(BigInt.fromString(tokenId)))
    );
    return event;
  };

  const createLegacyNameRegisteredEvent = (
    name: string,
    label: string,
    owner: string,
    cost: string,
    expires: string
  ): LegacyNameRegistered => {
    let mockEvent = newMockEvent();
    let event = new LegacyNameRegistered(
      mockEvent.address,
      mockEvent.logIndex,
      mockEvent.transactionLogIndex,
      mockEvent.logType,
      mockEvent.block,
      mockEvent.transaction,
      mockEvent.parameters,
      mockEvent.receipt
    );
    event.parameters = new Array();
    event.parameters.push(new ethereum.EventParam("name", ethereum.Value.fromString(name)));
    event.parameters.push(
      new ethereum.EventParam("label", ethereum.Value.fromFixedBytes(Bytes.fromHexString(label)))
    );
    event.parameters.push(
      new ethereum.EventParam("owner", ethereum.Value.fromAddress(Address.fromString(owner)))
    );
    event.parameters.push(
      new ethereum.EventParam("cost", ethereum.Value.fromUnsignedBigInt(BigInt.fromString(cost)))
    );
    event.parameters.push(
      new ethereum.EventParam("expires", ethereum.Value.fromUnsignedBigInt(BigInt.fromString(expires)))
    );
    return event;
  };

  const createLegacyNameRenewedEvent = (
    name: string,
    label: string,
    cost: string,
    expires: string
  ): LegacyNameRenewed => {
    let mockEvent = newMockEvent();
    let event = new LegacyNameRenewed(
      mockEvent.address,
      mockEvent.logIndex,
      mockEvent.transactionLogIndex,
      mockEvent.logType,
      mockEvent.block,
      mockEvent.transaction,
      mockEvent.parameters,
      mockEvent.receipt
    );
    event.parameters = new Array();
    event.parameters.push(new ethereum.EventParam("name", ethereum.Value.fromString(name)));
    event.parameters.push(
      new ethereum.EventParam("label", ethereum.Value.fromFixedBytes(Bytes.fromHexString(label)))
    );
    event.parameters.push(
      new ethereum.EventParam("cost", ethereum.Value.fromUnsignedBigInt(BigInt.fromString(cost)))
    );
    event.parameters.push(
      new ethereum.EventParam("expires", ethereum.Value.fromUnsignedBigInt(BigInt.fromString(expires)))
    );
    return event;
  };

  const createNameRegisteredEventForBaseRegistrar = (
    id: string,
    owner: string,
    expires: string
  ): NameRegistered => {
    let mockEvent = newMockEvent();
    let event = new NameRegistered(
      mockEvent.address,
      mockEvent.logIndex,
      mockEvent.transactionLogIndex,
      mockEvent.logType,
      mockEvent.block,
      mockEvent.transaction,
      mockEvent.parameters,
      mockEvent.receipt
    );
    event.parameters = new Array();
    event.parameters.push(
      new ethereum.EventParam("id", ethereum.Value.fromSignedBigInt(BigInt.fromString(id)))
    );
    event.parameters.push(
      new ethereum.EventParam("owner", ethereum.Value.fromAddress(Address.fromString(owner)))
    );
    event.parameters.push(
      new ethereum.EventParam("expires", ethereum.Value.fromSignedBigInt(BigInt.fromString(expires)))
    );
    return event;
  };

  beforeAll(() => {
    setEthOwner();
  });

  test("handleNameRenewed updates Registration/Domain expiryDate and writes history", () => {
    const labelhash =
      "0xf0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0";
    const labelhashAsInt =
      "108980789870415242751596221184647442685430573802955824978313020242741769072880";

    const newNewOwnerEvent = createNewOwnerEvent(
      ETH_NODE.toHexString(),
      labelhash,
      DEFAULT_OWNER
    );
    handleNewOwner(newNewOwnerEvent);

    let newRegistrationEvent = createNameRegisteredEventForBaseRegistrar(
      labelhashAsInt,
      DEFAULT_OWNER,
      "1610000000"
    );
    handleNameRegistered(newRegistrationEvent);

    let domainId = Bytes.fromByteArray(
      crypto.keccak256(concat(ETH_NODE, Bytes.fromHexString(labelhash)))
    );

    const renewedEvent = createNameRenewedEvent(labelhashAsInt, "1620000000");
    handleNameRenewed(renewedEvent);

    let fetchedRegistration = Registration.load(labelhash)!;
    assert.assertTrue(
      fetchedRegistration.expiryDate.equals(BigInt.fromString("1620000000"))
    );

    // Domain.expiryDate includes the 90-day grace period, matching
    // handleNameRegistered's own convention for the same field.
    let fetchedDomain = Domain.load(domainId.toHexString())!;
    assert.assertTrue(
      fetchedDomain.expiryDate!.equals(BigInt.fromString("1627776000"))
    );

    let eventId = createLegacyEventID(renewedEvent);
    assert.fieldEquals(
      "NameRenewed",
      eventId,
      "registration",
      Bytes.fromHexString(labelhash).toHexString()
    );
    assert.fieldEquals("NameRenewed", eventId, "expiryDate", "1620000000");
  });

  test("handleNameRenewed processes a second, later renewal on the same Registration correctly", () => {
    const labelhash =
      "0x9090909090909090909090909090909090909090909090909090909090909090";
    const labelhashAsInt =
      "65388473922249145650957732710788465611258344281773494986987812145645061443728";

    const newNewOwnerEvent = createNewOwnerEvent(
      ETH_NODE.toHexString(),
      labelhash,
      DEFAULT_OWNER
    );
    handleNewOwner(newNewOwnerEvent);

    let newRegistrationEvent = createNameRegisteredEventForBaseRegistrar(
      labelhashAsInt,
      DEFAULT_OWNER,
      "1610000000"
    );
    handleNameRegistered(newRegistrationEvent);

    let domainId = Bytes.fromByteArray(
      crypto.keccak256(concat(ETH_NODE, Bytes.fromHexString(labelhash)))
    );

    handleNameRenewed(createNameRenewedEvent(labelhashAsInt, "1620000000"));
    handleNameRenewed(createNameRenewedEvent(labelhashAsInt, "1630000000"));

    // The second renewal's absolute expiry must win outright -- expiryDate
    // is always a flat overwrite from the event's own value, not additive,
    // so two renewals must not double-count anything.
    let fetchedRegistration = Registration.load(labelhash)!;
    assert.assertTrue(
      fetchedRegistration.expiryDate.equals(BigInt.fromString("1630000000"))
    );
    let fetchedDomain = Domain.load(domainId.toHexString())!;
    assert.assertTrue(
      fetchedDomain.expiryDate!.equals(BigInt.fromString("1637776000"))
    );
  });

  test("renewal to (near) uint64 max, as the Graveyard does for parked migrated names, clamps Domain/Registration expiry to the max supported timestamp", () => {
    const labelhash =
      "0x6c5d5b0a5a3b0d1a9f1d0ecb49d8b0f2f6d4d0d9b7c8a6e5f4a3b2c1d0e9f8a7";
    const labelhashAsInt =
      "49014732762036582799472617395683540772314031190514023815906079271141442713767";

    handleNewOwner(
      createNewOwnerEvent(ETH_NODE.toHexString(), labelhash, DEFAULT_OWNER)
    );
    handleNameRegistered(
      createNameRegisteredEventForBaseRegistrar(
        labelhashAsInt,
        DEFAULT_OWNER,
        "1610000000"
      )
    );
    let domainId = Bytes.fromByteArray(
      crypto.keccak256(concat(ETH_NODE, Bytes.fromHexString(labelhash)))
    );

    // Graveyard.renew: uint64.max - block.timestamp - GRACE_PERIOD, so the
    // registrar's `expires` lands at uint64.max - 90 days.
    handleNameRenewed(
      createNameRenewedEvent(labelhashAsInt, "18446744073701775615")
    );

    let fetchedRegistration = Registration.load(labelhash)!;
    assert.assertTrue(
      fetchedRegistration.expiryDate.equals(BigInt.fromString("253402300799"))
    );
    let fetchedDomain = Domain.load(domainId.toHexString())!;
    assert.assertTrue(
      fetchedDomain.expiryDate!.equals(BigInt.fromString("253402300799"))
    );
  });

  test("renewal of a wrapped .eth name through the BaseRegistrar also refreshes WrappedDomain.expiryDate (#89)", () => {
    const labelhash =
      "0x7a1c3f9b2e4d6c8a0b5e7f9d1c3a5b7d9e1f3a5c7b9d1e3f5a7c9b1d3e5f7a9c";
    const labelhashAsInt =
      "55232078238660091151951310067410837771020824850170498950951524275750789151388";

    handleNewOwner(
      createNewOwnerEvent(ETH_NODE.toHexString(), labelhash, DEFAULT_OWNER)
    );
    handleNameRegistered(
      createNameRegisteredEventForBaseRegistrar(
        labelhashAsInt,
        DEFAULT_OWNER,
        "1610000000"
      )
    );
    let domainId = Bytes.fromByteArray(
      crypto.keccak256(concat(ETH_NODE, Bytes.fromHexString(labelhash)))
    ).toHexString();

    // Wrapped at the original expiry (+90d), as NameWrapped would record.
    let wrappedDomain = new WrappedDomain(domainId);
    wrappedDomain.domain = domainId;
    wrappedDomain.owner = DEFAULT_OWNER;
    wrappedDomain.fuses = 196608;
    wrappedDomain.expiryDate = BigInt.fromString("1617776000");
    wrappedDomain.save();

    handleNameRenewed(createNameRenewedEvent(labelhashAsInt, "1700000000"));

    // 1700000000 + 90d
    assert.fieldEquals("Domain", domainId, "expiryDate", "1707776000");
    assert.fieldEquals("WrappedDomain", domainId, "expiryDate", "1707776000");
    assert.fieldEquals("Registration", labelhash, "expiryDate", "1700000000");
  });

  test("handleNameTransferred updates Registration/Domain registrant, writes history, and no-ops for an unregistered tokenId", () => {
    const labelhash =
      "0xe1e1e1e1e1e1e1e1e1e1e1e1e1e1e1e1e1e1e1e1e1e1e1e1e1e1e1e1e1e1e1e1";
    const labelhashAsInt =
      "102169490503514290079621457360606977517591162940271085917168456477570408505825";
    const newOwner = "0xF0205A3A3b2A69De6Dbf7f01ED13B2108B2c4321";

    const newNewOwnerEvent = createNewOwnerEvent(
      ETH_NODE.toHexString(),
      labelhash,
      DEFAULT_OWNER
    );
    handleNewOwner(newNewOwnerEvent);

    let newRegistrationEvent = createNameRegisteredEventForBaseRegistrar(
      labelhashAsInt,
      DEFAULT_OWNER,
      "1610000000"
    );
    handleNameRegistered(newRegistrationEvent);

    let domainId = Bytes.fromByteArray(
      crypto.keccak256(concat(ETH_NODE, Bytes.fromHexString(labelhash)))
    );

    const transferEvent = createTransferEvent(
      DEFAULT_OWNER,
      newOwner,
      labelhashAsInt
    );
    handleNameTransferred(transferEvent);

    assert.fieldEquals(
      "Registration",
      Bytes.fromHexString(labelhash).toHexString(),
      "registrant",
      Address.fromString(newOwner).toHexString()
    );
    assert.fieldEquals(
      "Domain",
      domainId.toHexString(),
      "registrant",
      Address.fromString(newOwner).toHexString()
    );

    let eventId = createLegacyEventID(transferEvent);
    assert.fieldEquals("NameTransferred", eventId, "newOwner", Address.fromString(newOwner).toHexString());

    // A second, later transfer on the same already-registered name -- a
    // completely ordinary real-world sequence (a name gets resold) -- must
    // keep updating registrant correctly, not just work once.
    const thirdOwner = "0x1234567890123456789012345678901234567890";
    const secondTransferEvent = createTransferEvent(
      newOwner,
      thirdOwner,
      labelhashAsInt
    );
    handleNameTransferred(secondTransferEvent);

    assert.fieldEquals(
      "Registration",
      Bytes.fromHexString(labelhash).toHexString(),
      "registrant",
      Address.fromString(thirdOwner).toHexString()
    );
    assert.fieldEquals(
      "Domain",
      domainId.toHexString(),
      "registrant",
      Address.fromString(thirdOwner).toHexString()
    );

    // No Registration exists at all for this tokenId -- must no-op, not crash.
    const unregisteredLabelhash =
      "0xa5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5";
    const unregisteredLabelhashAsInt =
      "74924293035910479391722402064445116846233519489532129672590201416884966237605";
    const unregisteredTransferEvent = createTransferEvent(
      DEFAULT_OWNER,
      newOwner,
      unregisteredLabelhashAsInt
    );
    handleNameTransferred(unregisteredTransferEvent);

    assert.notInStore("Registration", Bytes.fromHexString(unregisteredLabelhash).toHexString());
  });

  test("handleNameRegisteredByLegacyController sets labelName/cost via the shared setNamePreimage path", () => {
    const labelhash =
      "0xd2d2d2d2d2d2d2d2d2d2d2d2d2d2d2d2d2d2d2d2d2d2d2d2d2d2d2d2d2d2d2d2";
    const labelhashAsInt =
      "95358191136613337407646693536566512349751752077586346856023892712399047938770";
    const label = "legacyreg";

    const newNewOwnerEvent = createNewOwnerEvent(
      ETH_NODE.toHexString(),
      labelhash,
      DEFAULT_OWNER
    );
    handleNewOwner(newNewOwnerEvent);

    let newRegistrationEvent = createNameRegisteredEventForBaseRegistrar(
      labelhashAsInt,
      DEFAULT_OWNER,
      "1610000000"
    );
    handleNameRegistered(newRegistrationEvent);

    const legacyEvent = createLegacyNameRegisteredEvent(
      label,
      labelhash,
      DEFAULT_OWNER,
      "1000000000",
      "1610000000"
    );
    handleNameRegisteredByLegacyController(legacyEvent);

    let fetchedRegistration = Registration.load(labelhash)!;
    assert.assertTrue(fetchedRegistration.labelName == label);
    assert.assertTrue(
      fetchedRegistration.cost!.equals(BigInt.fromString("1000000000"))
    );
  });

  test("setNamePreimage's no-Domain branch: no crash, no Registration created, when the Domain was never registered at all", () => {
    const labelhash =
      "0xb4b4b4b4b4b4b4b4b4b4b4b4b4b4b4b4b4b4b4b4b4b4b4b4b4b4b4b4b4b4b4b4";
    const label = "nodomain";

    // No handleNewOwner, no handleNameRegistered -- the Domain this label
    // would hash to genuinely doesn't exist anywhere in the store.
    const legacyEvent = createLegacyNameRegisteredEvent(
      label,
      labelhash,
      DEFAULT_OWNER,
      "1000000000",
      "1610000000"
    );
    handleNameRegisteredByLegacyController(legacyEvent);

    assert.notInStore("Registration", Bytes.fromHexString(labelhash).toHexString());
  });

  test("setNamePreimage's no-Registration branch: updates Domain.labelName but creates no Registration", () => {
    const labelhash =
      "0xb7b7b7b7b7b7b7b7b7b7b7b7b7b7b7b7b7b7b7b7b7b7b7b7b7b7b7b7b7b7b7b7";
    const label = "noregistration";

    // handleNewOwner creates the Domain (via ENSRegistry), but
    // handleNameRegistered (which creates the Registration) is deliberately
    // skipped -- setNamePreimage should still update the Domain's
    // labelName/name, but has nothing to attach cost/labelName to on the
    // Registration side, so none gets created.
    const newNewOwnerEvent = createNewOwnerEvent(
      ETH_NODE.toHexString(),
      labelhash,
      DEFAULT_OWNER
    );
    handleNewOwner(newNewOwnerEvent);

    const legacyEvent = createLegacyNameRegisteredEvent(
      label,
      labelhash,
      DEFAULT_OWNER,
      "1000000000",
      "1610000000"
    );
    handleNameRegisteredByLegacyController(legacyEvent);

    let domainId = Bytes.fromByteArray(
      crypto.keccak256(concat(ETH_NODE, Bytes.fromHexString(labelhash)))
    );
    assert.fieldEquals("Domain", domainId.toHexString(), "labelName", label);
    assert.notInStore("Registration", Bytes.fromHexString(labelhash).toHexString());
  });

  test("handleNameRenewedByLegacyController updates cost via the shared setNamePreimage path", () => {
    const labelhash =
      "0xc3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3";
    const labelhashAsInt =
      "88546891769712384735671929712526047181912341214901607794879328947227687371715";
    const label = "legacyrenew";

    const newNewOwnerEvent = createNewOwnerEvent(
      ETH_NODE.toHexString(),
      labelhash,
      DEFAULT_OWNER
    );
    handleNewOwner(newNewOwnerEvent);

    let newRegistrationEvent = createNameRegisteredEventForBaseRegistrar(
      labelhashAsInt,
      DEFAULT_OWNER,
      "1610000000"
    );
    handleNameRegistered(newRegistrationEvent);

    const legacyRegisteredEvent = createLegacyNameRegisteredEvent(
      label,
      labelhash,
      DEFAULT_OWNER,
      "1000000000",
      "1610000000"
    );
    handleNameRegisteredByLegacyController(legacyRegisteredEvent);

    const legacyRenewedEvent = createLegacyNameRenewedEvent(
      label,
      labelhash,
      "2000000000",
      "1620000000"
    );
    handleNameRenewedByLegacyController(legacyRenewedEvent);

    let fetchedRegistration = Registration.load(labelhash)!;
    assert.assertTrue(fetchedRegistration.labelName == label);
    assert.assertTrue(
      fetchedRegistration.cost!.equals(BigInt.fromString("2000000000"))
    );
  });
});
