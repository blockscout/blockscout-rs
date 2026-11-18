# Changelog

Version labels match the Subgraph Studio deployments of the mainnet subgraph
`5XqPmWe6gjyrJtFn9cLy237i4cWw2j9HcUJEXsP5qGtH`.

## v2.3.5 (2026-11-17)

Not deployed yet.

### Fixed

- One link could make a handler write an unbounded number of paths. A name's
  subregistry can be any address, so a registry can be linked under its own
  names or along a chain of registries, and every namespace a registry was
  projected under cost one path per name: 20 registrations across four
  registries gave 780 paths, and a registry with three names pointing back at
  it about 64 million. A registry's names are now projected under its
  canonical namespace (the name it declares with `setParent`, reached through
  that parent's canonical name) and at most 8 other namespaces, 16 in all,
  and one event backfills at most 10,000 names (the rest follows over later
  events, see below). A namespace that is refused stays active with
  `materialized: false`; emitting the link again retries it. The canonical
  namespace does not compete with aliases, so pointing other names at a
  registry cannot keep its own names out. The ETHRegistry is projected
  under names of the RootRegistry only, and the RootRegistry under none: a
  name pointed at the ETHRegistry would otherwise repeat every later `.eth`
  name and everything below it, so that each name in the registry of
  `foo.eth` counted several times against what may sit below `foo.eth` and
  took that registry's alias places. Whether a namespace is canonical is
  re-evaluated for the registries below when a registry declares its parent
  late, so one that had declared its own parent first no longer stays an
  alias, and is projected if it was refused as one. That walk covers at most
  100,000 names per event. Adds
  `ENSv2Registry.materializedNamespaces`, `ENSv2Namespace.materialized` and
  `ENSv2Namespace.canonical`. 1,754 of the registries on mainnet declare a
  parent, and in the 23 sampled the declaration preceded the link in 22.
- Nothing limited how many names could sit below one name, and removing a
  link, an unregistration or a re-registration retires all of them in the
  handler of that one event. A handler cannot release memory, and retiring a
  name took about 25 KB of the 4 GiB it has (measured in matchstick), so
  beyond roughly 160,000 names the handler could never complete and the
  subgraph would stop for good. It did not take a large registry to get
  there: a name pointed at the ETHRegistry collected every later `.eth`
  registration and migration under it, for one transaction. The names below
  every name are now counted, at every depth (new entity `ENSv2SubtreeSize`,
  per path and per slot), and a name that would take a name above it past
  100,000 is not projected: it has its `ENSv2NameSlot` and no `ENSv2NamePath`
  or `Domain`. Names directly under `eth` are exempt, since only ENS
  governance can change that link; every other TLD is limited like any
  name. The retire
  loop now loads three entities per name instead of seven and writes the
  parent's `subdomainCount` once per run of siblings, which brings a name
  down to about 11 KB, so a full 100,000 stays near 1 GiB. It also walks
  only what is live: `ENSv2NamespacePathIndex` used to be append-only, so
  names registered and unregistered in turn lengthened the list a
  retirement has to walk without ever exceeding the limit on live names. It
  is now the list of a namespace's active paths (`ENSv2Namespace.pathCount`
  long; a path retired on its own gives its position to the last one), and
  each path records its position in `ENSv2NamePath.namespaceIndex`. The
  same goes for `ENSv2SlotPathIndex` and `ENSv2NamePath.slotIndex`: a path
  that another slot takes over leaves the old slot's list. Two things only
  a contract that emits whatever it likes can do are refused outright,
  because each made a slot's list grow with every event: an event that gives
  an existing slot another labelhash is ignored (a registry derives the
  token id from the label), and a second slot carrying the same labelhash
  does not take a name that is live under the first.
  What a name costs to retire also depends on what hangs off it and on how
  long it is, so the count gives a subregistry projected under a name two
  units (a name with a resolver and a subregistry costs 32 to 50 KB), and a
  name longer than 255 bytes or more than 127 labels deep is not projected
  at all (TLDs and `label.eth` names themselves excepted): every name below
  a name repeats it, and a 10,000-character label took the cost of each
  name below it to 88 KB.
- A registry with more than 10,000 names when it was linked was never
  projected: the link was refused as a whole, and emitting it again met the
  same limit. That is the case of a large subname issuer that registers its
  names in a new registry and then points its name at it. Such a registry is
  now admitted and projected 10,000 names per event: what one event does not
  reach is queued (new entities `ENSv2BackfillQueue` and
  `ENSv2BackfillTask`) and taken up by the registry events that follow, in
  any registry, or by emitting the link again. Adds
  `ENSv2Namespace.backfilled`, `backfillCursor` and `backfillQueued`, and
  `ENSv2NameSlot.registryIndex`. While a namespace is not finished, the
  ENSv1 rows below its parent name are cleared as for a refused link. The
  10,000 is a budget, not a count of names: counting a name on its
  ancestors costs a load and a save per level, so a name is charged one unit
  plus one for every 16 levels above it, and a subregistry found on it one
  more for every 8. A full event then stays near 1.5 GiB of handler memory
  at any depth; without that, 10,000 names 128 levels deep took more than
  4 GiB. What an event retires also comes off its backfill budget, so a name
  that swaps one large registry for another retires in that event and
  backfills in the next.
- Removing a link and making it again repeated the projection of every
  registry below it, up to the backfill limit, for two events. A registry's
  names are now projected at most 32 times under its canonical namespace and
  32 times under aliases over its whole life, re-links higher up included;
  after that a link of that kind is recorded but not projected. Adds
  `ENSv2Registry.canonicalMaterializations` and `aliasMaterializations`.
- A record update on a PermissionedResolver refreshed every name linked to
  the record, and rebuilt the record's snapshot for each one. The source has
  no address filter, so any contract could emit `Linked` N times and an
  update M times for N x M work. One event now refreshes the first 100 names
  linked to the record (and, for a default record, the first 100 names
  pointed at the resolver), and builds the snapshot once. A name beyond that
  is never given a snapshot from the record, so its legacy `Resolver` and
  `Domain.resolvedAddress` are empty, never out of date; the resolver is
  flagged with `ENSv2Resolver.legacyProjectionTruncated`. Adds
  `ENSv2ResolverAttachment.index`. This replaces the earlier decision to
  have no cap. Whether a record is the resolver's default is read from the
  root link itself, so a record linked to the root node after 100 other
  nodes still carries its updates to the names that rely on it. A record's
  snapshot lists at most 1,000 coin types and 1,000 text keys
  (`Resolver.coinTypes`, `Resolver.texts`): a record keeps an index row for
  every one it was ever given, and each update walked them all. The
  resolved address is read by coin type and does not depend on the list. A
  text key longer than 255 characters is left out of the list.
- Names more than 16 registries below a link that was removed stayed live,
  with their resolver and address. Retiring names now has no depth limit
  (it works through a list instead of recursing); only projection is limited
  to 16 registries per event.
- ENSv1 subnames of a name registered in ENSv2 kept their ENSv1 resolver and
  address. ENSv2 resolves a name through its registries, so once the parent
  is registered in ENSv2 (migrated, or registered again after it lapsed), an
  ENSv1 subname that was not created again in ENSv2 is answered by the
  parent's ENSv2 resolver or does not resolve at all. On mainnet 163 such
  subnames under 24 migrated names showed an address the Universal Resolver
  no longer returns (157 return the zero address, 6 revert). When a name is
  projected, every ENSv1 `Domain` row below it, at any depth, now loses its
  resolver and resolved address, and ENSv1 `NewResolver` events for those
  names are history-only from then on. Adds
  `ENSv2NamePath.legacySubtreeVoided`. The sweep visits at most 100,000
  rows, and checks that against the number of child rows a name has ever
  had (new entity `DomainChildRows`) before it reads them, since the read
  loads deleted rows too. The 100,000 is shared by all the sweeps of one
  event: a backfill can project thousands of names, each with ENSv1 rows
  below it. A locked NameWrapper name that
  migrated into a WrapperRegistry keeps its emancipated subnames that have
  not migrated, which still resolve through ENSv1, until they migrate, leave
  the NameWrapper (new entity `ENSv2LegacyCutoff`), have their label
  reserved in the WrapperRegistry, or the name stops pointing at the
  WrapperRegistry its migration linked (another WrapperRegistry does not
  count: each serves the children of one name).
- A link that is refused by one of the limits above left the ENSv1 `Domain`
  rows of the names behind it showing their ENSv1 resolver and address.
  Every ENSv1 row below the linked name is now cleared, with the same mark.
  For a registered name the fix above has already done this; it matters for
  names under the RootRegistry and for locked migrations.
- A registration that a WrapperRegistry makes in itself (an emancipated
  subname migrating into its parent's registry) was not recorded as a
  migration. `ENSv2NameSlot.migratedFromV1` and
  `ENSv2LabelRegistered.isV1Migration` are now true for it.
- Retiring a name scanned every namespace the name had ever had below it, so
  pointing a name at many addresses and then unregistering and registering
  it repeatedly multiplied the work. A name has one subregistry at a time:
  the namespace linked now is kept in `ENSv2NamePath.activeNamespace` and is
  the only one followed. Linking a namespace retires any other still active
  under the same name, and a `SubregistryUpdated` no longer links under a
  path that another registry's slot has taken over.
- A name pointed at a resolver before the resolver had any link was not
  remembered, so a default record it was later given through an unlink
  (`Linked` to record 0) was never refreshed. It is now remembered at that
  point.
- Registering a name again left the previous holder's subnames live.
  `_register()` overwrites the entry's subregistry and emits
  `SubregistryUpdated` only for a non-zero value, so the names of the old
  subregistry stayed active, and a later link did not retire them either.
  `LabelRegistered` now retires them and deactivates the old link.
- A registry that was not deployed through VerifiableFactory got an
  `ENSv2Registry` row when a name pointed at it, but no data source, so none
  of its names were indexed. A registry now also gets its data source from
  its own first `LabelRegistered`, `LabelReserved` or `ParentUpdated`,
  through the new addressless `ENSv2RegistryDiscovery` data source. Being
  named by another registry does not start one, since any address can be
  named. Adds `ENSv2Registry.indexed`.
- `Registration.cost` read 0 for every name renewed since the ENSv2 launch.
  ETHRenewerV1, which renews the names still reserved for migration, takes
  its payment in an ERC-20 token and then calls
  `WrappedEthRegistrarController.renew(label, 0)` to sync the NameWrapper;
  the controller emits `NameRenewed` with cost 0, and anyone can trigger the
  same for any name through `syncWrapper`. A renewal with cost 0 no longer
  overwrites the cost. A native ENSv2 registration over a lapsed ENSv1
  registration now clears the cost instead of keeping the old one.
- `Domain.subdomainCount` drifted, below zero for some names. The original
  mapping, kept until now, took a child off its parent on every save of a
  row in the deleted state (no owner, no resolver, no children), so a second
  event for an already deleted subname (another `NewOwner` to the zero
  address, a `Transfer`, a `NewResolver`) cost the parent a second child, and
  it counted a subname only at its first `NewOwner`, so one that was deleted
  and created again was never counted again. The count now changes only when
  a name becomes live or stops being so, in both directions, and the change
  carries upward when the parent crosses the same line. The ENSv1 handlers,
  the ENSv2 projection and retirement, every write that sets or clears a
  row's owner or resolver, and the sweeps that take a resolver off ENSv1
  rows all go through the same functions (`src/subdomainCount.ts`). One consequence on the ENSv2 side: a retired
  name that still has live rows below it keeps counting in its parent until
  they are gone, as a deleted ENSv1 name always has.
- A transfer of value 0 changed the owner shown for a name.
  `ERC1155Singleton._update` moves a token only when the value is above 0
  and emits `TransferSingle` or `TransferBatch` either way, so a holder could
  have a registry emit a transfer of their own name to any address, and
  `Domain.owner`, `Domain.registrant`, `Registration.registrant` and the
  native slot and token followed it. Zero-value transfers are now ignored.
  A `TransferBatch` is followed for at most 1,000 distinct token ids: the
  log can list far more than a real batch could move, and each id walks the
  slot's paths.

Requires a full re-index.

## v2.3.4 (2026-11-17)

Supersedes v2.3.3, which was tagged but not deployed.

### Fixed

- A name could select a PermissionedResolver's address before the proxy was
  deployed. v2.3.3 only remembered names on a resolver it already knew to be
  record-based, so a default record set after deployment never reached such
  a name. A name selected through an ENSv2 registry (`ResolverUpdated`, or
  the late-link backfill) is now remembered whatever the resolver turns out
  to be, except the ENSV1Resolver that names reserved for migration point
  at. An ENSv1 name that selects a PermissionedResolver before it is deployed
  is still not tracked. Two mainnet resolvers were selected before
  deployment; neither has a default record, so no name was affected.
- A registry's `implementation` was set only from `ProxyDeployed`. Registries
  are UUPS proxies, and contracts-v2 v1.0.1 announces a new WrapperRegistry
  implementation, which existing wrapper registries would receive in place. The registry mapping now handles ERC1967
  `Upgraded(address)` and keeps `implementation` current, so
  implementation-specific behaviour (resolver and subregistry clearing on
  unregister) follows an upgrade. A registry keeps its kind; one not yet
  classified takes the kind of its new implementation.

### Changed

- Test for contracts-v2 v1.0.1's registration event order (roles granted
  before the mint: `LabelRegistered`, `TokenResource`, `EACRolesChanged`,
  `TransferSingle`). The mappings already handled it.

Requires a full re-index.

## v2.3.3 (2026-11-16)

Supersedes v2.3.2, which was tagged but not deployed.

### Fixed

- Names below a name that stops resolving stayed live. In the deployed
  registries `unregister()` sets the name's expiry to now and
  `getSubregistry()` returns zero for an expired name, so nothing below an
  unregistered name resolves; nor do the names of a subregistry its parent
  replaced. Only the first level was deactivated (and on unregister only for
  the fixed UserRegistry). Deactivating a path now deactivates the
  namespaces linked below it and their paths, recursively (up to 16 levels),
  and retires their `Domain` rows. Re-linking restores them through the
  v2.3.2 backfill. No mainnet name was affected as of block 26,426,095.
- A PermissionedResolver answers for a node without its own record from its
  default record (`_recordIds[0]`, the record linked to the root node),
  including a node unlinked to record 0. The projection ignored it, so
  `Resolver.addr` and `Domain.resolvedAddress` stayed empty. It now follows
  the default record, and a change to it reaches every name on that resolver
  that has no record of its own. Adds `ENSv2Resolver.recordBased`,
  `ENSv2Resolver.attachmentCount`, `ENSv2ResolverAttachment` and
  `ENSv2ResolverAttachmentIndex`. Two mainnet names were affected
  (ethbeggar.eth, walletours.eth).
- v2.3.2's backfill could allocate a namespace index from a stale registry
  snapshot when a registry is linked below itself, overwriting indexes and
  rolling back `namespaceCount`, so later registrations missed names. Index
  allocation now reads the current counter.

Requires a full re-index.

## v2.3.2 (2026-11-16)

### Fixed

- Names registered in a registry before that registry was linked under a
  parent ("late-linked") were never projected: they had no `ENSv2NamePath`
  and no `Domain` row, while the registry they replaced had its rows
  deleted. This is how subnames are copied into a new registry (deploy it,
  register the names, then point the parent at it), the planned recovery
  for registries on the affected UserRegistry implementations. Linking a
  registry now backfills every name already `REGISTERED` there, with its
  resolver, and recurses into the registries those names link to (up to 16
  levels). The backfill runs only when the namespace is new or was inactive.
  18 mainnet names were affected as of block 26,425,585 (for example
  support.v1.orivonid.eth and preview.app.navik.eth). Adds
  `ENSv2Registry.slotCount` and `ENSv2RegistrySlotIndex`.
- On a registry running the fixed UserRegistry implementation (contracts-v2
  #501: mainnet `0x86013382…25cb`, Sepolia `0x0822d199…a126`), `unregister()`
  also clears the name's resolver and subregistry without emitting events.
  The slot now clears both too, and the subregistry's namespaces are
  unlinked. Registries on older implementations keep both, as on chain.
- The later UserRegistry implementations (`0x75ff…51bb`, `0x6648…0Dc0`,
  `0x43d9…b23D`, `0x8601…25cb` on mainnet, `0x0822…a126` on Sepolia) are
  classified as `USER` instead of `UNKNOWN`.

Requires a full re-index.

## v2.3.1 (2026-11-13)

### Fixed

- An ENSv1 name whose registry resolver is set to a PermissionedResolver now
  takes its records from the record linked to it. A PermissionedResolver has
  no node-keyed events: its records arrive through `Linked` and the
  `*Updated` events, which only project onto a name while the registry
  already points at that resolver. Before, an ENSv1 `NewResolver` created an
  empty legacy `Resolver` row, so a name linked and populated before the
  registry switched kept `Resolver.addr` and `Domain.resolvedAddress` null.
  A name that switched away and back kept the address from before the
  switch. ENSv1 `NewResolver` now attaches the resolver the same way as
  ENSv2's `ResolverUpdated` (`attachDomainResolver`). Classic resolvers
  behave as before. ENSv2 names were not affected. On mainnet no ENSv1 name
  pointed at a PermissionedResolver with a linked record as of block
  26,405,541, so no existing row changes.

## v2.3.0 (2026-11-13)

### Added

- `Domain.registryToken`: the name's current token in its ENSv2 registry
  (an `ENSv2Token`, whose id is the registry address followed by the uint256
  token id), so consumers can link a live ENSv2 name to its token without
  joining the native entities. Set when the registration's `TokenResource`
  arrives and on `TokenRegenerated`, cleared when the name is unregistered or
  its namespace unlinked; null for names that are not live in ENSv2. Their
  ENSv1 BaseRegistrar / NameWrapper tokens are retired or unrelated.

Requires a full re-index.

## v2.2.1 (2026-11-13)

### Fixed

- A `Linked` event with a malformed DNS-encoded name (empty, a length byte
  that runs past the end, or no root terminator) aborted the mapping with an
  out-of-range read. The PermissionedResolver data source has no address, so
  any contract could emit such an event and halt the subgraph. `decodeName`
  now returns null for malformed input; the link is indexed with
  `nameDecoded = null`. No such event exists on mainnet so far.
- `Resolver.addr` and `Domain.resolvedAddress` of a name on a
  PermissionedResolver now follow `addr()` on chain: the coin type 60
  address, or the ENSIP-19 default EVM address (coin type `2147483648`) when
  coin type 60 is unset or empty. A non-empty coin type 60 value, including
  the zero address, still overrides the default. On mainnet 25 records rely
  on the default (block 26,404,320).

Requires a full re-index (the second fix changes already indexed rows).

## v2.2.0 (2026-11-11)

The legacy `Domain` / `Registration` / `WrappedDomain` entities are now
correct on their own: no consumer needs to know about the ENSv2 Graveyard or
migration contracts. Three rules (`src/legacyGovernance.ts`):

1. **A sink never becomes a holder.** ENSv1 events that hand a registry
   record, BaseRegistrar token or NameWrapper token to the Graveyard, the
   GraveyardRepairer or a migration controller only write their history rows.
   A `.eth` name keeps its owner (a migration's ENSv2 registration follows in
   the same transaction), a retired `.eth` name keeps its whole lapsed state
   including its last wrapped holder, and a subname moved to the Graveyard is
   deleted with the ENSv1 convention.
2. **ENSv2 governs a live ENSv2 name.** While a node has an active ENSv2 path
   with a `REGISTERED` slot, ENSv1 registry, BaseRegistrar and NameWrapper
   events for it only write history. Later `Graveyard.clear()` calls over the
   retired v1 record of a migrated or natively registered name no longer
   overwrite its owner, resolver or expiry.
3. **The ENSv2 projection is complete.** Every registration, native or
   migration, in any registry writes the ENSv2 holder into `owner` and
   `registrant`, removes v1 NameWrapper state, resets the resolver to the
   ENSv2 one, and sets `registrationDate` for native registrations.
   Transfers and renewals sync in every registry, not only the ETH registry.
   Unregistration and namespace unlinks delete the projected row. A path a
   new subregistry takes over is rebound to its new slot.

### Changed

- `Domain.owner` of a migrated name is its ENSv2 holder, no longer the
  Graveyard (28,322 names on mainnet at block 26,389,002), and the 25 locked
  migrations lose their v1 `WrappedDomain`.
- `Domain.owner` of a swept expired `.eth` name is its last ENSv1 owner, no
  longer the Graveyard (about 950,000 names).

### Fixed (mainnet counts at block 26,389,002)

- 3,361 swept wrapped `.eth` names lost their last wrapped holder.
- 848 retired subnames: 702 read as live under their old wrapped holder, 146
  as Graveyard-owned. They now read as deleted.
- 680 live ENSv2 names had their owner overwritten with the Graveyard and 625
  their resolver wiped by a later v1 clear (for example visigoth.eth,
  bmw-000.tpa.eth).
- 190 native ENSv2 `.eth` names kept a stale ENSv1 resolver, and 583 native
  ENSv2 names kept stale v1 wrapped state (6 with the Graveyard as
  `wrappedOwner`).
- 1,500 of 2,678 native ENSv2 `.eth` registrations showed the old v1
  `registrationDate`.
- 6 transfers and 8 renewals in user registries never reached the legacy row.
- 35 unregistered ENSv2 subnames still read as live, 8 of them with a path
  left active after their parent switched subregistry.
- GraveyardRepairer (contracts-v2, not yet deployed) is recognised as a
  retirement registrant once its address is configured.

### Corrected

- The v2.1.3 entry below says none of its fixes had produced wrong data on
  mainnet. That was wrong: v2.1.3 restored `resolvedAddress` on 451 ENSv2
  names that point at a v1 PublicResolver, and reclassified 2 registries from
  `UNKNOWN` to `USER`.

Requires a full re-index.

## v2.1.3 (2026-11-09)

Hardening of three edge cases raised in review of the ENSv2 mappings. None of
them has produced wrong data on mainnet (checked against a fully synced
mainnet database on 2026-11-09), so no re-index is required.

### Fixed

- An ENSv2 registry `ResolverUpdated` pointing a name at a classic node-keyed
  resolver (PublicResolverV2 and friends, which have no `ENSv2ResolverLink`)
  no longer wipes the `addr`/`contentHash`/`texts`/`coinTypes` snapshot that
  the ENSv1 resolver handlers keep on the legacy `Resolver` row, nor nulls
  `Domain.resolvedAddress`. It now behaves like the ENSv1 `NewResolver`
  handler: the snapshot is kept and its `addr` is copied to
  `Domain.resolvedAddress`. The snapshot is still cleared when a link exists
  and is explicitly inactive (`linkToRecord(name, 0)`), and an active link
  still projects the native record.
- `ProxyDeployed` now reclassifies an `ENSv2Registry` row that an earlier
  `SubregistryUpdated`/`ParentUpdated` created as `UNKNOWN` (possible only
  across data-source start blocks or on a redeployed testnet) once the
  implementation is known, instead of leaving `kind` frozen at `UNKNOWN`.
- The gated 2017-registry handlers (`handleNewResolverOldRegistry`,
  `handleNewTTLOldRegistry`, `handleTransferOldRegistry`) log a warning and
  skip an event for a node with no `Domain` instead of aborting indexing on a
  non-null assertion. Both registries are indexed from their deployment
  blocks, so this does not occur on mainnet or Sepolia.

## v2.1.2 (2026-11-09)

### Fixed

- Expired ENSv1 `.eth` names retired by the ENSv2 Graveyard no longer read as
  live registrations owned by the Graveyard. `Graveyard.clear()` re-registers
  an expired name to the Graveyard with a near-`2^64` expiry (burn, mint,
  registry `NewOwner`, `NameRegistered`, resolver cleared); on mainnet the
  first sweep (blocks 26,297,075 to 26,372,896) covered 946,001 names, about a
  fifth of all `.eth` registrations, which then showed `registrant` =
  Graveyard and `expiryDate` = the year-9999 clamp. A `NameRegistered` to the
  Graveyard now keeps the lapsed `registrant`, `registrationDate` and
  `expiryDate` on `Registration` and `Domain` (the name is expired and
  available, as it looked before the sweep), the burn and the mint to the
  Graveyard leave `registrant` alone, stale `WrappedDomain` state is cleared,
  and the `NameTransferred`/`NameRegistered` history rows are still written.
  `Domain.owner` still follows the registry to the Graveyard, as for migrated
  names. Migrations (a live token moved by a migration controller) are
  unchanged. Requires a full re-index.

## v2.1.1 (2026-10-31)

### Fixed

- The "never expires" clamp on `Domain`, `Registration` and `WrappedDomain`
  `expiryDate` is now `253402300799` (9999-12-31T23:59:59Z) instead of
  `8640000000000` (13 Sep 275760, the JavaScript `Date` maximum). The old
  value fits PostgreSQL but not Rust's `chrono` (max year 262142), so Rust
  consumers decoding it as a timestamp panicked (Blockscout's BENS). Year
  9999 is representable by every mainstream date type. Native ENSv2 entities
  and event entities still carry the raw on-chain value.

## v2.1.0 (2026-10-31)

### Fixed

- `Domain.wrappedOwner` and the `WrappedDomain` entity are now cleared when a wrapped name leaves the NameWrapper without a `NameUnwrapped` event (replaces #78):
  - a registry `NewOwner` / `Transfer` that moves the name away from the NameWrapper (parent `setSubnodeOwner` on a subname without `PARENT_CANNOT_CONTROL`, `BaseRegistrar.reclaim`);
  - a native ENSv2 `.eth` registration of a name whose ENSv1 wrapped registration had expired (expiry emits no event on either contract).
  Moves to the ENSv2 Graveyard or a migration controller are the migration's voiding step and keep their wrapped state for `correctMigratedLegacyOwner`. A NameWrapper `TransferSingle` / `TransferBatch` of a token whose name carries no wrapped state (a detached token) now writes only the `WrappedTransfer` history row instead of reviving `wrappedOwner`. On mainnet 509 names were affected at the time of the fix.

## v2.0.3 (2026-10-30)

- `Account.ensv2PrimaryNames`: derived link from an account to its ENSIP-19
  default primary name (#25, #38).
- README: self-hosting notes (graph-node version, `ens_names` rainbow table
  for ENSv1 subname labels, duplicate log-index RPCs, pruning) (#62, #30,
  #77, #68).
- Refresh `WrappedDomain.expiryDate` when a wrapped `.eth` name is renewed
  through the BaseRegistrar. The NameWrapper normalises its expiry on read
  (registrar expiry + 90 days) without emitting `ExpiryExtended`, so the
  subgraph value went stale after every such renewal: 75,043 wrapped names
  were affected on mainnet at 2026-10-30 (#89).

## v2.0.2 (2026-10-30)

- Classify registries deployed against the ENS app's mainnet
  `UserRegistryImpl` (`0xC927bCaA…`) as `USER`. On v2.0.1 those 286
  registries read `kind: UNKNOWN`; their names were otherwise indexed and
  projected correctly. `ENSv2Registry.kind` is the only field affected.

## v2.0.1 (2026-10-30), published to production

First ENSv2 release. Indexes ENSv2 registries, namespaces, slots, paths,
resources, tokens, registrations, resolvers, roles and ENSIP-19 primary names
alongside the unchanged ENSv1 entities, and projects ENSv2 state into the
legacy `Domain` / `Registration` / `Resolver` / `WrappedDomain` entities.

Behaviour changes for consumers of the legacy entities:

- `Domain.owner` of a migrated name stays the retired ENSv1 registry owner
  (the Graveyard for unwrapped names, the NameWrapper for wrapped ones).
  `registrant`, `wrappedOwner` and the ENSv2 entities carry the real holder.
  List an address's names by `registrant`/`wrappedOwner`/`resolvedAddress` as
  well as `owner`.
- `Domain.expiryDate` for migrated and ENSv2-native names includes the ENSv2
  grace period (28 days), not ENSv1's 90 days. Read `Registration.expiryDate`
  for the raw expiry.
- Never-expiring names (`2^64-1` on chain) are written as `8640000000000` on
  `Domain`, `Registration` and `WrappedDomain` `expiryDate`; native ENSv2
  entities and event entities keep the raw value.
- `indexerHints: prune: auto` is enabled: time-travel queries
  (`block: { number }`) are only served for recent blocks.
- Root paths (`eth`, `reverse`) are native ENSv2 entities only and never
  overwrite the ENSv1 TLD `Domain` rows.
- Legacy `Domain.subdomainCount` is maintained for projected names
  (incremented on registration, decremented on unregister, restored on
  re-registration).

Deployment: specVersion 1.3.0 / apiVersion 0.0.9 (graph-node >= 0.38);
CI pinned to `ubuntu-24.04` for the matchstick binary.

## v1.1.0 and earlier

ENSv1-only subgraph. See git history.
