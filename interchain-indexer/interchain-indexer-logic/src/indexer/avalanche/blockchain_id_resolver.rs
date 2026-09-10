// SPDX-License-Identifier: LicenseRef-Blockscout

use std::time::Duration;

use anyhow::{Context, Result, anyhow};
use moka::future::Cache;

use crate::{
    InterchainDatabase,
    avalanche_data_api::{
        AvalancheDataApiClient, AvalancheDataApiClientSettings, AvalancheDataApiNetwork,
        DataApiError, GetBlockchainByIdResponse,
    },
    protocol_metadata::UnresolvedReason,
};

/// Cache key: (blockchain_id bytes)
type CacheKey = [u8; 32];

/// How long a confirmed-unresolvable blockchain ID stays in the negative
/// cache. Bounds pickup delay on the *destination* path only — see
/// `resolve_destination`'s docs. A module constant, deliberately not an
/// ENV/config setting: this task does not expand the config surface.
const UNRESOLVED_BLOCKCHAIN_ID_TTL: Duration = Duration::from_secs(600);
const UNRESOLVED_BLOCKCHAIN_ID_CACHE_CAPACITY: u64 = 10_000;

/// Outcome of resolving an Avalanche `blockchain_id` to an EVM chain id.
/// Unlike a plain `Result`, an expected "no such destination" is data
/// (`Unresolved`), not a processing failure.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Resolution {
    Resolved(i64),
    Unresolved(UnresolvedReason),
}

/// Error type for the closure driving the positive cache's `try_get_with`.
/// `moka` does not cache errors, so a confirmed-unresolvable outcome is
/// still routed through this `Err` arm and recognized by the caller from its
/// variant — never written to the positive cache, and never itself logged
/// here (see `resolve_lookup`'s docs on why).
enum LookupError {
    Unresolved(UnresolvedReason),
    Other(anyhow::Error),
}

impl From<anyhow::Error> for LookupError {
    fn from(err: anyhow::Error) -> Self {
        Self::Other(err)
    }
}

/// Classifies a single Data API call's outcome per the resolution table:
/// a response with an `evmChainId` resolves; a response without one, or a
/// confirmed not-found error, are `Unresolved` — both are data, not
/// failures. Any other error is not classifiable here at all
/// (`DataApiClassification::Propagate`) and must be surfaced as a genuine
/// failure by the caller. Pure — no I/O — so this is unit-tested directly
/// against fixtures, without a cache, a database, or HTTP.
#[derive(Debug, PartialEq, Eq)]
enum DataApiClassification {
    Resolution(Resolution),
    Propagate,
}

fn classify_data_api_result(
    result: &Result<GetBlockchainByIdResponse, DataApiError>,
) -> DataApiClassification {
    match result {
        Ok(resp) => DataApiClassification::Resolution(match resp.evm_chain_id {
            Some(chain_id) => Resolution::Resolved(chain_id),
            None => Resolution::Unresolved(UnresolvedReason::NoChainId),
        }),
        Err(DataApiError::BlockchainNotFound { .. }) => DataApiClassification::Resolution(
            Resolution::Unresolved(UnresolvedReason::UnknownIdentifier),
        ),
        Err(_) => DataApiClassification::Propagate,
    }
}

#[derive(Clone)]
pub struct BlockchainIdResolver {
    data_api: AvalancheDataApiClient,
    network: AvalancheDataApiNetwork,
    /// Resolved mappings. Shared by both paths, no TTL — as before this
    /// task. `try_get_with` gives single-flight deduplication for
    /// concurrent lookups of the same key on *either* path.
    resolved: Cache<CacheKey, i64>,
    /// Confirmed unresolvability. Read and written ONLY by
    /// `resolve_destination` — the source path (`resolve`) never touches
    /// this field. This separation is an invariant, not an optimization: a
    /// shared cache would let the source path's retry return a stale
    /// negative result for the whole TTL even after the Data API recovers,
    /// which changes retry *behavior*, not just its outcome.
    unresolved: Cache<CacheKey, UnresolvedReason>,
    db: InterchainDatabase,
}

impl BlockchainIdResolver {
    pub fn new(settings: AvalancheDataApiClientSettings, db: InterchainDatabase) -> Self {
        let network = settings.network;
        Self {
            data_api: AvalancheDataApiClient::from_settings(settings),
            network,
            resolved: Cache::new(10_000),
            unresolved: Cache::builder()
                .max_capacity(UNRESOLVED_BLOCKCHAIN_ID_CACHE_CAPACITY)
                .time_to_live(UNRESOLVED_BLOCKCHAIN_ID_TTL)
                .build(),
            db,
        }
    }

    /// The Avalanche Data API network this resolver looks blockchain IDs up
    /// against, taken from the same settings it was built from.
    pub(crate) fn network(&self) -> AvalancheDataApiNetwork {
        self.network
    }

    fn parse_key(blockchain_id: &[u8]) -> Result<CacheKey> {
        blockchain_id.try_into().map_err(|_| {
            anyhow!(
                "expected 32-byte blockchain_id, got {}",
                blockchain_id.len()
            )
        })
    }

    /// Shared DB → Data API lookup, used by both `resolve` and
    /// `resolve_destination`. A positive result is cached in `resolved`, as
    /// before. A confirmed-unresolvable outcome is *not* written to any
    /// cache here — that is the caller's decision (only
    /// `resolve_destination` writes the negative cache); this method only
    /// classifies and reports it. An unrecognized Data API error is not
    /// logged here: `error-handling.md` requires logging at the point of
    /// handling, not along the propagation path, or the same failure would
    /// be logged twice.
    async fn resolve_lookup(
        &self,
        key: CacheKey,
        process_unknown_chains: bool,
    ) -> Result<Resolution> {
        let force_add_chain = process_unknown_chains;

        let this = self.clone();
        let result = self
            .resolved
            .try_get_with(key, async move {
                if let Some(chain_id) = this
                    .db
                    .get_avalanche_icm_chain_id_by_blockchain_id(&key)
                    .await
                    .context("failed to query avalanche_icm_blockchain_ids")?
                {
                    return Ok::<i64, LookupError>(chain_id);
                }

                let resp = this.data_api.get_blockchain_by_id(&key).await;
                let chain_id = match classify_data_api_result(&resp) {
                    DataApiClassification::Resolution(Resolution::Resolved(chain_id)) => chain_id,
                    DataApiClassification::Resolution(Resolution::Unresolved(reason)) => {
                        return Err(LookupError::Unresolved(reason));
                    }
                    DataApiClassification::Propagate => {
                        let err = resp.expect_err("Propagate classification implies Err");
                        return Err(anyhow!(err)
                            .context("failed to fetch blockchain info from Avalanche Data API")
                            .into());
                    }
                };
                let resp = resp.expect("Resolved classification implies Ok");
                let chain_name = resp.blockchain_name;
                let response_blockchain_id = resp.blockchain_id;

                let persist = force_add_chain
                    || this
                        .db
                        .get_chain_by_id(
                            u64::try_from(chain_id)
                                .map_err(|_| anyhow!("evm_chain_id does not fit u64"))?,
                        )
                        .await
                        .context("failed to query chains")?
                        .is_some();

                if persist {
                    // Ensure FK target exists when we persist the mapping.
                    if let Err(err) = this
                        .db
                        .ensure_chain_exists(chain_id, Some(chain_name.clone()), None)
                        .await
                    {
                        tracing::warn!(
                            err = ?err,
                            chain_id,
                            blockchain_id = %response_blockchain_id,
                            blockchain_name = ?chain_name,
                            "failed to ensure chains row for discovered evmChainId"
                        );
                    }

                    if let Err(err) = this
                        .db
                        .upsert_avalanche_icm_blockchain_id(key.to_vec(), chain_id)
                        .await
                    {
                        tracing::warn!(
                            err = ?err,
                            chain_id,
                            blockchain_id = %response_blockchain_id,
                            blockchain_name = ?chain_name,
                            "failed to upsert avalanche_icm_blockchain_ids row"
                        );
                    }
                }

                Ok::<i64, LookupError>(chain_id)
            })
            .await;

        match result {
            Ok(chain_id) => Ok(Resolution::Resolved(chain_id)),
            Err(arc_err) => match &*arc_err {
                LookupError::Unresolved(reason) => Ok(Resolution::Unresolved(*reason)),
                // `{:#}` unwraps the whole anyhow chain into one string —
                // `Arc<anyhow::Error>` cannot be unwrapped back into an
                // owned `anyhow::Error` under moka's single-flight sharing,
                // so re-render it in full rather than losing the chain to
                // `Display`'s top-level-only rendering (as `.to_string()`
                // would).
                LookupError::Other(err) => Err(anyhow!("{err:#}")),
            },
        }
    }

    /// Resolve an Avalanche `blockchain_id` (32 bytes) for the **source**
    /// path: semantics are unchanged from before this task — `Err` →
    /// `indexer_failures` → replay. This method never reads or writes the
    /// negative cache, so a retry always re-consults the DB/Data API rather
    /// than returning a cached negation from a previous destination-path
    /// lookup.
    pub(crate) async fn resolve(
        &self,
        blockchain_id: &[u8],
        process_unknown_chains: bool,
    ) -> Result<i64> {
        let key = Self::parse_key(blockchain_id)?;
        match self.resolve_lookup(key, process_unknown_chains).await? {
            Resolution::Resolved(chain_id) => Ok(chain_id),
            Resolution::Unresolved(reason) => {
                Err(anyhow!("blockchain id could not be resolved: {reason:?}"))
            }
        }
    }

    /// Resolve an Avalanche `blockchain_id` (32 bytes) for the
    /// **destination** path: an expected, confirmed inability to resolve is
    /// a result (`Resolution::Unresolved`), not an error. Wraps
    /// `resolve_lookup` with a negative cache so a destination that keeps
    /// failing the same way within `UNRESOLVED_BLOCKCHAIN_ID_TTL` does not
    /// re-hit the DB/Data API for every send in a batch.
    ///
    /// The negative-cache hit bypasses the DB entirely, so a mapping that
    /// becomes available in `avalanche_icm_blockchain_ids` via another
    /// bridge is not picked up instantly on this path — only within the
    /// TTL. That delay is harmless here: the message is already saved as
    /// data, no block is held in the failure ledger, and a later event on
    /// the same key (or a replay) re-consolidates it.
    pub(crate) async fn resolve_destination(
        &self,
        blockchain_id: &[u8],
        process_unknown_chains: bool,
    ) -> Result<Resolution> {
        let key = Self::parse_key(blockchain_id)?;

        if let Some(reason) = self.unresolved.get(&key).await {
            return Ok(Resolution::Unresolved(reason));
        }

        let resolution = self.resolve_lookup(key, process_unknown_chains).await?;
        if let Resolution::Unresolved(reason) = resolution {
            self.unresolved.insert(key, reason).await;
        }
        Ok(resolution)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        avalanche_data_api::{AvalancheDataApiClientSettings, AvalancheDataApiNetwork},
        test_utils,
    };

    fn not_found_response() -> Result<GetBlockchainByIdResponse, DataApiError> {
        Err(DataApiError::BlockchainNotFound {
            network: AvalancheDataApiNetwork::Mainnet,
        })
    }

    fn other_error_response() -> Result<GetBlockchainByIdResponse, DataApiError> {
        Err(DataApiError::Status {
            status: reqwest::StatusCode::INTERNAL_SERVER_ERROR,
            message: "boom".to_string(),
        })
    }

    fn resolved_response(
        evm_chain_id: Option<i64>,
    ) -> Result<GetBlockchainByIdResponse, DataApiError> {
        Ok(GetBlockchainByIdResponse {
            blockchain_id: "cb58-id".to_string(),
            blockchain_name: "test-chain".to_string(),
            evm_chain_id,
        })
    }

    #[test]
    fn classify_resolved_when_evm_chain_id_present() {
        assert_eq!(
            classify_data_api_result(&resolved_response(Some(43114))),
            DataApiClassification::Resolution(Resolution::Resolved(43114))
        );
    }

    #[test]
    fn classify_unresolved_no_chain_id_when_evm_chain_id_absent() {
        assert_eq!(
            classify_data_api_result(&resolved_response(None)),
            DataApiClassification::Resolution(Resolution::Unresolved(UnresolvedReason::NoChainId))
        );
    }

    #[test]
    fn classify_unresolved_unknown_identifier_on_blockchain_not_found() {
        assert_eq!(
            classify_data_api_result(&not_found_response()),
            DataApiClassification::Resolution(Resolution::Unresolved(
                UnresolvedReason::UnknownIdentifier
            ))
        );
    }

    #[test]
    fn classify_propagates_any_other_data_api_error() {
        assert_eq!(
            classify_data_api_result(&other_error_response()),
            DataApiClassification::Propagate
        );
    }

    /// The key invariant of the two-cache split: a `resolve_destination`
    /// miss caches `Unresolved`, but a subsequent `resolve` (source path)
    /// lookup for the same key must not see that cached negation — it must
    /// still consult the DB/API and succeed once the underlying source has
    /// healed. Swaps the underlying source (the DB row / cache state)
    /// directly rather than waiting out the TTL, so the test is
    /// deterministic and does not depend on real time or network access.
    #[tokio::test]
    #[ignore = "needs database"]
    async fn destination_negative_cache_does_not_leak_into_source_path() -> Result<()> {
        let db_guard = test_utils::init_db("resolver_negative_cache_separation").await;
        let db = db_guard.client();
        let interchain_db = InterchainDatabase::new(db.clone());

        let settings = AvalancheDataApiClientSettings {
            network: AvalancheDataApiNetwork::Mainnet,
            api_key: None,
        };
        let resolver = BlockchainIdResolver::new(settings, interchain_db.clone());
        assert_eq!(resolver.network(), AvalancheDataApiNetwork::Mainnet);

        let blockchain_id = [0x42u8; 32];

        // The underlying source has since healed: a mapping now exists in
        // `avalanche_icm_blockchain_ids` (as another bridge might create).
        interchain_db
            .ensure_chain_exists(999_999, Some("healed-chain".to_string()), None)
            .await?;
        interchain_db
            .upsert_avalanche_icm_blockchain_id(blockchain_id.to_vec(), 999_999)
            .await?;

        // Simulate a prior destination-path lookup that confirmed
        // unresolvability, without touching the network: `unresolved` is a
        // private field, reachable directly from this same-module test.
        resolver
            .unresolved
            .insert(blockchain_id, UnresolvedReason::UnknownIdentifier)
            .await;

        // Destination path: the negative-cache hit is checked before the DB,
        // so it still reports `Unresolved` even though a healed mapping now
        // exists — that's the accepted bounded-delay tradeoff, not a bug.
        let destination_result = resolver.resolve_destination(&blockchain_id, true).await?;
        assert_eq!(
            destination_result,
            Resolution::Unresolved(UnresolvedReason::UnknownIdentifier)
        );

        // Source path: must ignore the negative cache entirely and go
        // straight to the DB, finding the healed mapping.
        let source_result = resolver.resolve(&blockchain_id, true).await?;
        assert_eq!(source_result, 999_999);

        Ok(())
    }

    /// End-to-end test for the resolver.
    ///
    /// - Boots a real Postgres test DB (migrations applied)
    /// - Instantiates `BlockchainIdResolver`
    /// - Makes a real call to Avalanche Data API
    /// - Asserts the mapping is persisted
    ///
    /// Intentionally `#[ignore]` because it requires network access and can be
    /// flaky/rate-limited.
    #[tokio::test]
    #[ignore = "requires network access to data-api.avax.network and a postgres test db"]
    async fn resolves_native_id_to_chain_id_8021_and_persists_mapping() -> Result<()> {
        let native_id = "0xd32cc4660bcf8fa7971589f666fddb5ab22aee7e75dcb30b19829a65d4fb0063";

        let bytes = alloy::hex::decode(native_id.trim_start_matches("0x"))
            .context("native_id must be hex")?;
        anyhow::ensure!(bytes.len() == 32, "blockchainID must be 32 bytes");

        let db_guard = test_utils::init_db("resolver_resolves_8021").await;
        let db = db_guard.client();
        let interchain_db = InterchainDatabase::new(db.clone());

        // Optional API key support.
        let api_key = std::env::var("AVALANCHE_GLACIER_API_KEY")
            .ok()
            .or_else(|| std::env::var("AVALANCHE_DATA_API_KEY").ok())
            .filter(|s| !s.trim().is_empty());

        let settings = AvalancheDataApiClientSettings {
            network: AvalancheDataApiNetwork::Mainnet,
            api_key,
        };

        let resolver = BlockchainIdResolver::new(settings, interchain_db.clone());

        let resolved = resolver.resolve(&bytes, true).await?;
        anyhow::ensure!(resolved == 8021, "expected 8021, got {:?}", resolved);

        let persisted = interchain_db
            .get_avalanche_icm_chain_id_by_blockchain_id(&bytes)
            .await?;
        anyhow::ensure!(
            persisted == Some(8021),
            "expected persisted 8021, got {:?}",
            persisted
        );

        Ok(())
    }
}
