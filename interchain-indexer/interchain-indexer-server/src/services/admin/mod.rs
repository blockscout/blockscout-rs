// SPDX-License-Identifier: LicenseRef-Blockscout

//! Operator write API: authenticated, audited changes that would otherwise
//! need manual SQL. See ADR-016.

mod audit;
mod error;
mod validation;

use crate::{
    auth::{Actor, WriteApiAuth},
    proto::{
        SetStatsAssetIconRequest, SetStatsAssetIconResponse, SetTokenIconRequest,
        SetTokenIconResponse, interchain_admin_service_server::InterchainAdminService,
    },
};
use anyhow::Context;
use audit::AuditedTx;
use error::AdminError;
use interchain_indexer_entity::sea_orm_active_enums::TokenType;
use interchain_indexer_logic::{
    InterchainDatabase, TokenInfoService,
    write_api::{self, StatsAssetIconChange},
};
use sea_orm::ActiveEnum;
use serde_json::json;
use std::{sync::Arc, time::Duration};
use validation::{parse_token_selector, resolve_icon_change, validate_reason};

/// How long an asset-icon change waits for a concurrent writer of the same
/// `stats_assets` row before it gives up with `ABORTED`.
pub(crate) const ASSET_ICON_LOCK_TIMEOUT: Duration = Duration::from_secs(5);

const SET_STATS_ASSET_ICON: &str = "SetStatsAssetIcon";
const SET_TOKEN_ICON: &str = "SetTokenIcon";

pub(crate) struct InterchainAdminServiceImpl {
    auth: Arc<WriteApiAuth>,
    db: Arc<InterchainDatabase>,
    /// Owns the token cache that serves `/transfers` and `/messages`; it is told
    /// to drop an entry after a token icon changes.
    token_info: Arc<TokenInfoService>,
}

impl InterchainAdminServiceImpl {
    pub(crate) fn new(
        auth: Arc<WriteApiAuth>,
        db: Arc<InterchainDatabase>,
        token_info: Arc<TokenInfoService>,
    ) -> Self {
        Self {
            auth,
            db,
            token_info,
        }
    }

    async fn apply_stats_asset_icon(
        &self,
        actor: &Actor,
        request: SetStatsAssetIconRequest,
    ) -> Result<SetStatsAssetIconResponse, AdminError> {
        let stats_asset_id = request.stats_asset_id;
        if stats_asset_id <= 0 {
            Err(AdminError::InvalidArgument(
                "stats_asset_id must be a positive integer".to_string(),
            ))?;
        }
        let icon_url = resolve_icon_change(request.icon_url.as_deref(), request.clear)?;
        let reason = validate_reason(&request.reason)?;

        // The normalized request: what was applied, not what was sent.
        let request_json = json!({
            "stats_asset_id": stats_asset_id,
            "icon_url": icon_url,
            "clear": icon_url.is_none(),
        });
        let audited = AuditedTx::begin(&self.db, actor, SET_STATS_ASSET_ICON, reason, request_json)
            .await
            .context("failed to begin the asset icon transaction")?;

        let change = match write_api::set_stats_asset_icon_tx(
            audited.tx(),
            stats_asset_id,
            icon_url.as_deref(),
            ASSET_ICON_LOCK_TIMEOUT,
        )
        .await
        {
            Ok(Some(change)) => change,
            // `audited` is dropped on every early exit, which rolls back.
            Ok(None) => Err(AdminError::NotFound(format!(
                "stats asset {stats_asset_id} not found; it may have been merged into another asset — re-read the bridged-tokens list"
            )))?,
            Err(err) if write_api::is_lock_not_available(&err) => Err(AdminError::Aborted(
                "asset is being updated by stats maintenance; retry".to_string(),
            ))?,
            Err(err) => Err(anyhow::Error::new(err).context(format!(
                "failed to set the icon of stats asset {stats_asset_id}"
            )))?,
        };

        let audit_id = audited
            .commit(stats_asset_icon_result_json(&change))
            .await
            .with_context(|| {
                format!("failed to commit the icon change of stats asset {stats_asset_id}")
            })?;

        tracing::info!(
            method = SET_STATS_ASSET_ICON,
            actor = %actor,
            audit_id,
            stats_asset_id,
            icon_url_after = ?change.after.icon_url,
            "write api change applied"
        );
        Ok(SetStatsAssetIconResponse {
            audit_id,
            icon_url_before: change.before.icon_url,
            icon_url_after: change.after.icon_url,
        })
    }

    async fn apply_token_icon(
        &self,
        actor: &Actor,
        request: SetTokenIconRequest,
    ) -> Result<SetTokenIconResponse, AdminError> {
        let chain_id = request.chain_id;
        let native = request.native == Some(true);
        let address = parse_token_selector(request.address.as_deref(), request.native)?;
        let icon_url = resolve_icon_change(request.icon_url.as_deref(), request.clear)?;
        let reason = validate_reason(&request.reason)?;
        // There is deliberately no "is this chain configured" check: a chain
        // without a `tokens` row simply has no token to change.
        let hex_or_native = match native {
            true => "native".to_string(),
            false => format!("0x{}", hex::encode(&address)),
        };

        // The normalized request: what was applied, not what was sent.
        let token = match native {
            true => json!({"native": true}),
            false => json!({"address": hex_or_native}),
        };
        let request_json = json!({
            "chain_id": chain_id,
            "token": token,
            "icon_url": icon_url,
            "clear": icon_url.is_none(),
        });
        let audited = AuditedTx::begin(&self.db, actor, SET_TOKEN_ICON, reason, request_json)
            .await
            .context("failed to begin the token icon transaction")?;

        // No `TokenInfoService` call in here: the transaction holds the `tokens`
        // row lock, and that service's per-key mutex is held by writers waiting
        // for the same row. Taking the mutex now would deadlock.
        let change = match write_api::set_token_icon_tx(
            audited.tx(),
            chain_id,
            &address,
            icon_url.as_deref(),
        )
        .await
        {
            Ok(Some(change)) => change,
            // `audited` is dropped on every early exit, which rolls back.
            Ok(None) => Err(AdminError::NotFound(
                "token not found: a token row must exist (seen in transfers) before its icon can be set"
                    .to_string(),
            ))?,
            Err(err) => Err(anyhow::Error::new(err).context(format!(
                "failed to set the icon of token {hex_or_native} on chain {chain_id}"
            )))?,
        };

        let icon_url_after = change.after.token_icon;
        let audit_id = audited
            .commit(json!({
                "icon_url_before": change.before,
                "icon_url_after": icon_url_after,
            }))
            .await
            .with_context(|| {
                format!(
                    "failed to commit the icon change of token {hex_or_native} on chain {chain_id}"
                )
            })?;

        // Strictly after the commit: the cache must not be refilled from the old
        // row, and the mutex must not be awaited while the row is locked. The
        // invalidation can wait for a slow request-time icon lookup, so it runs
        // in a spawned task: if the client disconnects and this future is
        // dropped meanwhile, dropping the handle does not cancel the task, and
        // the committed change still reaches the cache. The change is already
        // committed, so a failed task is logged, not returned.
        let token_info = self.token_info.clone();
        if let Err(err) = tokio::spawn(async move {
            token_info.invalidate_cached(chain_id, &address).await;
        })
        .await
        {
            tracing::error!(
                method = SET_TOKEN_ICON,
                actor = %actor,
                audit_id,
                chain_id,
                err = ?err,
                "token cache invalidation task failed"
            );
        }

        tracing::info!(
            method = SET_TOKEN_ICON,
            actor = %actor,
            audit_id,
            chain_id,
            native,
            address = %hex_or_native,
            icon_url_after = ?icon_url_after,
            "write api change applied"
        );
        Ok(SetTokenIconResponse {
            audit_id,
            icon_url_before: change.before,
            icon_url_after,
        })
    }
}

/// The audit `result`: enough to roll back (`icon_url_before`) and, after the
/// asset loses a merge, to find its successor through `member_tokens`.
fn stats_asset_icon_result_json(change: &StatsAssetIconChange) -> serde_json::Value {
    let member_tokens: Vec<_> = change
        .member_tokens
        .iter()
        .map(|member| {
            let token_address = (member.r#type != TokenType::Native)
                .then(|| format!("0x{}", hex::encode(&member.token_address)));
            json!({
                "chain_id": member.chain_id,
                "token_address": token_address,
                "type": member.r#type.to_value(),
            })
        })
        .collect();
    json!({
        "icon_url_before": change.before.icon_url,
        "icon_url_after": change.after.icon_url,
        "name": change.after.name,
        "symbol": change.after.symbol,
        "member_tokens": member_tokens,
    })
}

/// The single point where a failed request is logged and turned into a status.
fn finish<T>(
    method: &'static str,
    actor: &Actor,
    result: Result<T, AdminError>,
) -> Result<T, tonic::Status> {
    result.map_err(|err| {
        match &err {
            AdminError::Internal(_) => {
                tracing::error!(method, actor = %actor, err = ?err, "write api request failed")
            }
            _ => tracing::warn!(method, actor = %actor, err = %err, "write api request rejected"),
        }
        err.into()
    })
}

#[async_trait::async_trait]
impl InterchainAdminService for InterchainAdminServiceImpl {
    async fn set_stats_asset_icon(
        &self,
        request: tonic::Request<SetStatsAssetIconRequest>,
    ) -> Result<tonic::Response<SetStatsAssetIconResponse>, tonic::Status> {
        let actor = self.auth.authenticate(SET_STATS_ASSET_ICON, &request)?;
        let result = self
            .apply_stats_asset_icon(&actor, request.into_inner())
            .await;
        finish(SET_STATS_ASSET_ICON, &actor, result).map(tonic::Response::new)
    }

    async fn set_token_icon(
        &self,
        request: tonic::Request<SetTokenIconRequest>,
    ) -> Result<tonic::Response<SetTokenIconResponse>, tonic::Status> {
        let actor = self.auth.authenticate(SET_TOKEN_ICON, &request)?;
        let result = self.apply_token_icon(&actor, request.into_inner()).await;
        finish(SET_TOKEN_ICON, &actor, result).map(tonic::Response::new)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::test_support::{auth_with_key, request_with_key};
    use blockscout_service_launcher::test_database::TestDbGuard;
    use interchain_indexer_entity::{
        chains, stats_asset_tokens, stats_assets, tokens, write_api_audit_log,
    };
    use interchain_indexer_logic::TokenInfoServiceSettings;
    use pretty_assertions::assert_eq;
    use sea_orm::{ActiveValue::Set, EntityTrait, QueryOrder, QuerySelect, TransactionTrait};
    use std::collections::HashMap;
    use tonic::{Code, metadata::AsciiMetadataValue};

    const KEY: &str = "test-key";
    const OLD_ICON: &str = "https://old.example/i.png";
    const GOOD_URL: &str = "https://example.com/i.png";
    const TOKEN_ICON: &str = "https://token.example/t.png";

    /// The token cache as the server wires it: no providers, no Blockscout URL.
    fn token_info_service(db: &Arc<InterchainDatabase>) -> Arc<TokenInfoService> {
        Arc::new(TokenInfoService::new(
            db.clone(),
            HashMap::new(),
            TokenInfoServiceSettings::default(),
        ))
    }

    struct Fixture {
        guard: TestDbGuard,
        service: InterchainAdminServiceImpl,
        token_info: Arc<TokenInfoService>,
        asset_id: i64,
    }

    /// An asset with an icon and two member tokens (an ERC-20 and a native one),
    /// plus a `tokens` row that must never change.
    async fn fixture(name: &str) -> Fixture {
        let guard = TestDbGuard::new::<migration::Migrator>(name).await;
        let db = Arc::new(InterchainDatabase::new(guard.client()));
        let conn = guard.client();
        let conn = conn.as_ref();

        chains::Entity::insert_many([1i64, 100].map(|id| chains::ActiveModel {
            id: Set(id),
            name: Set(format!("chain{id}")),
            ..Default::default()
        }))
        .exec(conn)
        .await
        .unwrap();
        let asset_id = stats_assets::Entity::insert(stats_assets::ActiveModel {
            name: Set(Some("USD Coin".to_string())),
            symbol: Set(Some("USDC".to_string())),
            icon_url: Set(Some(OLD_ICON.to_string())),
            ..Default::default()
        })
        .exec_with_returning(conn)
        .await
        .unwrap()
        .id;
        // Inserted out of chain order: the audit lists members by chain.
        stats_asset_tokens::Entity::insert_many([
            stats_asset_tokens::ActiveModel {
                stats_asset_id: Set(asset_id),
                chain_id: Set(100),
                token_address: Set(vec![0u8; 20]),
                r#type: Set(TokenType::Native),
                ..Default::default()
            },
            stats_asset_tokens::ActiveModel {
                stats_asset_id: Set(asset_id),
                chain_id: Set(1),
                token_address: Set(vec![0xabu8; 20]),
                r#type: Set(TokenType::Erc20),
                ..Default::default()
            },
        ])
        .exec(conn)
        .await
        .unwrap();
        tokens::Entity::insert_many([
            tokens::ActiveModel {
                chain_id: Set(1),
                address: Set(vec![0xabu8; 20]),
                r#type: Set(TokenType::Erc20),
                token_icon: Set(Some(TOKEN_ICON.to_string())),
                ..Default::default()
            },
            tokens::ActiveModel {
                chain_id: Set(100),
                address: Set(vec![0u8; 20]),
                r#type: Set(TokenType::Native),
                token_icon: Set(None),
                ..Default::default()
            },
        ])
        .exec(conn)
        .await
        .unwrap();

        let token_info = token_info_service(&db);
        Fixture {
            service: InterchainAdminServiceImpl::new(
                Arc::new(auth_with_key("ops_alice", KEY)),
                db,
                token_info.clone(),
            ),
            guard,
            token_info,
            asset_id,
        }
    }

    impl Fixture {
        fn request(
            &self,
            icon_url: Option<&str>,
            clear: Option<bool>,
            reason: &str,
        ) -> SetStatsAssetIconRequest {
            SetStatsAssetIconRequest {
                stats_asset_id: self.asset_id,
                icon_url: icon_url.map(str::to_string),
                clear,
                reason: reason.to_string(),
            }
        }

        async fn asset(&self) -> stats_assets::Model {
            stats_assets::Entity::find_by_id(self.asset_id)
                .one(self.guard.client().as_ref())
                .await
                .unwrap()
                .expect("asset exists")
        }

        async fn token_rows(&self) -> Vec<tokens::Model> {
            tokens::Entity::find()
                .all(self.guard.client().as_ref())
                .await
                .unwrap()
        }

        async fn audit_log(&self) -> Vec<write_api_audit_log::Model> {
            write_api_audit_log::Entity::find()
                .order_by_asc(write_api_audit_log::Column::Id)
                .all(self.guard.client().as_ref())
                .await
                .unwrap()
        }

        /// Calls the method as `KEY`.
        async fn call(
            &self,
            request: SetStatsAssetIconRequest,
        ) -> Result<SetStatsAssetIconResponse, tonic::Status> {
            self.service
                .set_stats_asset_icon(request_with_key(request, KEY))
                .await
                .map(tonic::Response::into_inner)
        }
    }

    #[tokio::test]
    #[ignore = "needs database"]
    async fn admin_handler_asset_icon_rejects_bad_auth_and_changes_nothing() {
        let fx = fixture("admin_handler_icon_bad_auth").await;
        let asset_before = fx.asset().await;
        let tokens_before = fx.token_rows().await;
        let valid_body = || fx.request(Some(GOOD_URL), None, "TICKET-1");

        let mut non_ascii = tonic::Request::new(valid_body());
        non_ascii.metadata_mut().insert(
            "x-api-key",
            AsciiMetadataValue::try_from(&[0xff_u8, b'k'][..]).unwrap(),
        );
        let empty_catalogue = InterchainAdminServiceImpl::new(
            Arc::new(WriteApiAuth::from_settings(&Default::default()).unwrap()),
            Arc::new(InterchainDatabase::new(fx.guard.client())),
            fx.token_info.clone(),
        );
        // `(label, response)`
        let outcomes = [
            (
                "no key",
                fx.service
                    .set_stats_asset_icon(tonic::Request::new(valid_body()))
                    .await,
            ),
            (
                "wrong key",
                fx.service
                    .set_stats_asset_icon(request_with_key(valid_body(), "wrong-key"))
                    .await,
            ),
            (
                "empty key",
                fx.service
                    .set_stats_asset_icon(request_with_key(valid_body(), ""))
                    .await,
            ),
            (
                "non-ASCII key",
                fx.service.set_stats_asset_icon(non_ascii).await,
            ),
            (
                "empty catalogue",
                empty_catalogue
                    .set_stats_asset_icon(request_with_key(valid_body(), KEY))
                    .await,
            ),
        ];
        for (label, outcome) in outcomes {
            let status = outcome.expect_err(label);
            assert_eq!(status.code(), Code::Unauthenticated, "{label}");
        }

        assert_eq!(fx.asset().await, asset_before);
        assert_eq!(fx.token_rows().await, tokens_before);
        assert_eq!(fx.audit_log().await.len(), 0);
    }

    /// Authentication comes first: a request without a valid key learns nothing
    /// about the validity of its body. No database is reachable here, so every
    /// request must also stop before it.
    #[tokio::test]
    async fn admin_handler_asset_icon_authenticates_before_validating() {
        let db = Arc::new(InterchainDatabase::new(Arc::new(
            sea_orm::DatabaseConnection::Disconnected,
        )));
        let service = InterchainAdminServiceImpl::new(
            Arc::new(auth_with_key("ops_alice", KEY)),
            db.clone(),
            token_info_service(&db),
        );
        let request =
            |icon_url: Option<&str>, clear: Option<bool>, reason: &str| SetStatsAssetIconRequest {
                stats_asset_id: 1,
                icon_url: icon_url.map(str::to_string),
                clear,
                reason: reason.to_string(),
            };
        let invalid_bodies = [
            (
                "http url",
                request(Some("http://example.com/i.png"), None, "r"),
            ),
            ("both fields", request(Some(GOOD_URL), Some(true), "r")),
            ("neither field", request(None, None, "r")),
            ("empty reason", request(Some(GOOD_URL), None, "")),
            (
                "non-positive id",
                SetStatsAssetIconRequest {
                    stats_asset_id: 0,
                    ..request(Some(GOOD_URL), None, "r")
                },
            ),
        ];

        for (label, body) in invalid_bodies {
            // Control: with a valid key the body really is rejected as invalid.
            let status = service
                .set_stats_asset_icon(request_with_key(body.clone(), KEY))
                .await
                .expect_err(label);
            assert_eq!(status.code(), Code::InvalidArgument, "{label}: control");

            for (key_label, unauthenticated) in [
                ("no key", tonic::Request::new(body.clone())),
                ("wrong key", request_with_key(body.clone(), "wrong-key")),
            ] {
                let status = service
                    .set_stats_asset_icon(unauthenticated)
                    .await
                    .expect_err(label);
                assert_eq!(
                    status.code(),
                    Code::Unauthenticated,
                    "{label} / {key_label}: authentication must come before validation"
                );
                assert_eq!(status.message(), "missing or invalid x-api-key");
            }
        }
    }

    #[tokio::test]
    #[ignore = "needs database"]
    async fn admin_handler_asset_icon_applies_and_writes_one_audit_row() {
        let fx = fixture("admin_handler_icon_applies").await;
        let tokens_before = fx.token_rows().await;

        // Both the URL and the reason are normalized before use.
        let response = fx
            .call(fx.request(Some("https://EXAMPLE.com/i.png"), None, "  TICKET-1  "))
            .await
            .unwrap();

        assert_eq!(response.icon_url_before.as_deref(), Some(OLD_ICON));
        assert_eq!(response.icon_url_after.as_deref(), Some(GOOD_URL));
        assert_eq!(fx.asset().await.icon_url.as_deref(), Some(GOOD_URL));
        assert_eq!(
            fx.token_rows().await,
            tokens_before,
            "tokens must not change"
        );

        let log = fx.audit_log().await;
        assert_eq!(log.len(), 1, "exactly one audit row per applied change");
        let row = &log[0];
        assert_eq!(row.id, response.audit_id);
        assert_eq!(row.actor, "ops_alice");
        assert_eq!(row.method, "SetStatsAssetIcon");
        assert_eq!(row.reason, "TICKET-1");
        assert_eq!(
            row.request,
            json!({"stats_asset_id": fx.asset_id, "icon_url": GOOD_URL, "clear": false})
        );
        assert_eq!(
            row.result,
            json!({
                "icon_url_before": OLD_ICON,
                "icon_url_after": GOOD_URL,
                "name": "USD Coin",
                "symbol": "USDC",
                "member_tokens": [
                    {
                        "chain_id": 1,
                        "token_address": format!("0x{}", "ab".repeat(20)),
                        "type": "erc20",
                    },
                    {"chain_id": 100, "token_address": null, "type": "native"},
                ],
            })
        );

        // Clearing records `before` for the rollback and stores NULL.
        let cleared = fx
            .call(fx.request(None, Some(true), "TICKET-1 revert"))
            .await
            .unwrap();
        assert_eq!(cleared.icon_url_before.as_deref(), Some(GOOD_URL));
        assert_eq!(cleared.icon_url_after, None);
        assert_eq!(fx.asset().await.icon_url, None);
        let log = fx.audit_log().await;
        assert_eq!(log.len(), 2);
        assert_eq!(
            log[1].request,
            json!({"stats_asset_id": fx.asset_id, "icon_url": null, "clear": true})
        );
        assert_eq!(log[1].result["icon_url_before"], json!(GOOD_URL));
        assert_eq!(log[1].result["icon_url_after"], json!(null));
        assert_eq!(
            fx.token_rows().await,
            tokens_before,
            "tokens must not change"
        );
    }

    #[tokio::test]
    #[ignore = "needs database"]
    async fn admin_handler_asset_icon_not_found_writes_no_audit() {
        let fx = fixture("admin_handler_icon_not_found").await;
        let mut request = fx.request(Some(GOOD_URL), None, "TICKET-1");
        request.stats_asset_id = fx.asset_id + 1000;

        let status = fx.call(request).await.expect_err("no such asset");
        assert_eq!(status.code(), Code::NotFound);
        assert!(status.message().contains("merged"), "{}", status.message());
        assert_eq!(fx.audit_log().await.len(), 0);
        assert_eq!(fx.asset().await.icon_url.as_deref(), Some(OLD_ICON));
    }

    #[tokio::test]
    #[ignore = "needs database"]
    async fn admin_handler_asset_icon_validation_errors_write_no_audit() {
        let fx = fixture("admin_handler_icon_validation").await;
        let mut non_positive_id = fx.request(Some(GOOD_URL), None, "TICKET-1");
        non_positive_id.stats_asset_id = 0;

        let requests = [
            (
                "http url",
                fx.request(Some("http://example.com/i.png"), None, "TICKET-1"),
            ),
            (
                "both fields",
                fx.request(Some(GOOD_URL), Some(true), "TICKET-1"),
            ),
            ("clear=false", fx.request(None, Some(false), "TICKET-1")),
            ("neither field", fx.request(None, None, "TICKET-1")),
            ("empty reason", fx.request(Some(GOOD_URL), None, "")),
            ("blank reason", fx.request(Some(GOOD_URL), None, "  \n ")),
            ("non-positive id", non_positive_id),
        ];
        for (label, request) in requests {
            let status = fx.call(request).await.expect_err(label);
            assert_eq!(status.code(), Code::InvalidArgument, "{label}");
        }

        assert_eq!(fx.audit_log().await.len(), 0);
        assert_eq!(fx.asset().await.icon_url.as_deref(), Some(OLD_ICON));
    }

    #[tokio::test]
    #[ignore = "needs database"]
    async fn admin_handler_asset_icon_lock_timeout_is_aborted_without_audit() {
        let fx = fixture("admin_handler_icon_lock_timeout").await;
        // A FOR UPDATE holder stands in for a long stats maintenance transaction.
        let holder = fx.guard.client().begin().await.unwrap();
        stats_assets::Entity::find_by_id(fx.asset_id)
            .lock_exclusive()
            .one(&holder)
            .await
            .unwrap()
            .expect("asset exists");

        let status = fx
            .call(fx.request(Some(GOOD_URL), None, "TICKET-1"))
            .await
            .expect_err("the lock wait must time out");
        holder.rollback().await.unwrap();

        assert_eq!(status.code(), Code::Aborted);
        assert!(status.message().contains("retry"), "{}", status.message());
        assert_eq!(fx.audit_log().await.len(), 0);
        assert_eq!(fx.asset().await.icon_url.as_deref(), Some(OLD_ICON));
    }

    const ERC20: [u8; 20] = [0xab; 20];

    fn erc20_address() -> String {
        format!("0x{}", hex::encode(ERC20))
    }

    fn token_request(
        chain_id: i64,
        address: Option<&str>,
        native: Option<bool>,
        icon_url: Option<&str>,
        clear: Option<bool>,
        reason: &str,
    ) -> SetTokenIconRequest {
        SetTokenIconRequest {
            chain_id,
            address: address.map(str::to_string),
            native,
            icon_url: icon_url.map(str::to_string),
            clear,
            reason: reason.to_string(),
        }
    }

    impl Fixture {
        /// Calls `SetTokenIcon` as `KEY`.
        async fn call_token(
            &self,
            request: SetTokenIconRequest,
        ) -> Result<SetTokenIconResponse, tonic::Status> {
            self.service
                .set_token_icon(request_with_key(request, KEY))
                .await
                .map(tonic::Response::into_inner)
        }

        async fn asset_tables(&self) -> (Vec<stats_assets::Model>, Vec<stats_asset_tokens::Model>) {
            let conn = self.guard.client();
            (
                stats_assets::Entity::find()
                    .all(conn.as_ref())
                    .await
                    .unwrap(),
                stats_asset_tokens::Entity::find()
                    .all(conn.as_ref())
                    .await
                    .unwrap(),
            )
        }

        /// What `/transfers` would show: the icon the token cache serves.
        async fn served_icon(&self, chain_id: i64, address: &[u8]) -> Option<String> {
            self.token_info
                .clone()
                .get_token_info(chain_id, address.to_vec())
                .await
                .unwrap()
                .token_icon
        }
    }

    #[tokio::test]
    #[ignore = "needs database"]
    async fn admin_handler_token_icon_rejects_bad_auth_and_changes_nothing() {
        let fx = fixture("admin_handler_token_icon_bad_auth").await;
        let tokens_before = fx.token_rows().await;
        let assets_before = fx.asset_tables().await;
        let address = erc20_address();
        let valid_body =
            || token_request(1, Some(&address), None, Some(GOOD_URL), None, "TICKET-1");

        let mut non_ascii = tonic::Request::new(valid_body());
        non_ascii.metadata_mut().insert(
            "x-api-key",
            AsciiMetadataValue::try_from(&[0xff_u8, b'k'][..]).unwrap(),
        );
        let empty_catalogue = InterchainAdminServiceImpl::new(
            Arc::new(WriteApiAuth::from_settings(&Default::default()).unwrap()),
            Arc::new(InterchainDatabase::new(fx.guard.client())),
            fx.token_info.clone(),
        );
        // `(label, response)`
        let outcomes = [
            (
                "no key",
                fx.service
                    .set_token_icon(tonic::Request::new(valid_body()))
                    .await,
            ),
            (
                "wrong key",
                fx.service
                    .set_token_icon(request_with_key(valid_body(), "wrong-key"))
                    .await,
            ),
            (
                "empty key",
                fx.service
                    .set_token_icon(request_with_key(valid_body(), ""))
                    .await,
            ),
            ("non-ASCII key", fx.service.set_token_icon(non_ascii).await),
            (
                "empty catalogue",
                empty_catalogue
                    .set_token_icon(request_with_key(valid_body(), KEY))
                    .await,
            ),
        ];
        for (label, outcome) in outcomes {
            let status = outcome.expect_err(label);
            assert_eq!(status.code(), Code::Unauthenticated, "{label}");
        }

        assert_eq!(fx.token_rows().await, tokens_before);
        assert_eq!(fx.asset_tables().await, assets_before);
        assert_eq!(fx.audit_log().await.len(), 0);
    }

    /// Authentication comes first for this method too, and no database is
    /// reachable here, so every request must stop before it.
    #[tokio::test]
    async fn admin_handler_token_icon_authenticates_before_validating() {
        let db = Arc::new(InterchainDatabase::new(Arc::new(
            sea_orm::DatabaseConnection::Disconnected,
        )));
        let service = InterchainAdminServiceImpl::new(
            Arc::new(auth_with_key("ops_alice", KEY)),
            db.clone(),
            token_info_service(&db),
        );
        let address = erc20_address();
        let invalid_bodies = [
            (
                "no selector",
                token_request(1, None, None, Some(GOOD_URL), None, "r"),
            ),
            (
                "both selectors",
                token_request(1, Some(&address), Some(true), Some(GOOD_URL), None, "r"),
            ),
            (
                "http url",
                token_request(
                    1,
                    Some(&address),
                    None,
                    Some("http://example.com/i.png"),
                    None,
                    "r",
                ),
            ),
            (
                "neither icon field",
                token_request(1, Some(&address), None, None, None, "r"),
            ),
            (
                "empty reason",
                token_request(1, Some(&address), None, Some(GOOD_URL), None, ""),
            ),
        ];

        for (label, body) in invalid_bodies {
            // Control: with a valid key the body really is rejected as invalid.
            let status = service
                .set_token_icon(request_with_key(body.clone(), KEY))
                .await
                .expect_err(label);
            assert_eq!(status.code(), Code::InvalidArgument, "{label}: control");

            for (key_label, unauthenticated) in [
                ("no key", tonic::Request::new(body.clone())),
                ("wrong key", request_with_key(body.clone(), "wrong-key")),
            ] {
                let status = service
                    .set_token_icon(unauthenticated)
                    .await
                    .expect_err(label);
                assert_eq!(
                    status.code(),
                    Code::Unauthenticated,
                    "{label} / {key_label}: authentication must come before validation"
                );
            }
        }
    }

    #[tokio::test]
    #[ignore = "needs database"]
    async fn admin_handler_token_icon_applies_writes_one_audit_row_and_refreshes_cache() {
        let fx = fixture("admin_handler_token_icon_applies").await;
        let address = erc20_address();
        let assets_before = fx.asset_tables().await;
        let native_before = fx
            .token_rows()
            .await
            .into_iter()
            .find(|row| row.chain_id == 100)
            .expect("native row");

        // Warm the cache: the old icon is what `/transfers` serves now.
        assert_eq!(fx.served_icon(1, &ERC20).await.as_deref(), Some(TOKEN_ICON));

        // Both the URL and the reason are normalized before use.
        let response = fx
            .call_token(token_request(
                1,
                Some(&address),
                None,
                Some("https://EXAMPLE.com/i.png"),
                None,
                "  TICKET-1  ",
            ))
            .await
            .unwrap();
        assert_eq!(response.icon_url_before.as_deref(), Some(TOKEN_ICON));
        assert_eq!(response.icon_url_after.as_deref(), Some(GOOD_URL));

        let log = fx.audit_log().await;
        assert_eq!(log.len(), 1, "exactly one audit row per applied change");
        let row = &log[0];
        assert_eq!(row.id, response.audit_id);
        assert_eq!(row.actor, "ops_alice");
        assert_eq!(row.method, "SetTokenIcon");
        assert_eq!(row.reason, "TICKET-1");
        assert_eq!(
            row.request,
            json!({
                "chain_id": 1,
                "token": {"address": address},
                "icon_url": GOOD_URL,
                "clear": false,
            })
        );
        assert_eq!(
            row.result,
            json!({"icon_url_before": TOKEN_ICON, "icon_url_after": GOOD_URL})
        );
        assert_eq!(
            fx.served_icon(1, &ERC20).await.as_deref(),
            Some(GOOD_URL),
            "the next read after the call sees the new icon without a restart"
        );

        // The native token is selected by `native`, not by an address.
        let native = fx
            .call_token(token_request(
                100,
                None,
                Some(true),
                Some(GOOD_URL),
                None,
                "TICKET-2",
            ))
            .await
            .unwrap();
        assert_eq!(native.icon_url_before, None);
        assert_eq!(native.icon_url_after.as_deref(), Some(GOOD_URL));
        let log = fx.audit_log().await;
        assert_eq!(log.len(), 2);
        assert_eq!(
            log[1].request,
            json!({
                "chain_id": 100,
                "token": {"native": true},
                "icon_url": GOOD_URL,
                "clear": false,
            })
        );
        assert_eq!(
            log[1].result,
            json!({"icon_url_before": null, "icon_url_after": GOOD_URL})
        );
        assert_eq!(
            fx.served_icon(100, &[0u8; 20]).await.as_deref(),
            Some(GOOD_URL)
        );

        // Clearing records `before` for the rollback and stores NULL.
        let cleared = fx
            .call_token(token_request(
                1,
                Some(&address),
                None,
                None,
                Some(true),
                "TICKET-1 revert",
            ))
            .await
            .unwrap();
        assert_eq!(cleared.icon_url_before.as_deref(), Some(GOOD_URL));
        assert_eq!(cleared.icon_url_after, None);
        let log = fx.audit_log().await;
        assert_eq!(log.len(), 3);
        assert_eq!(
            log[2].request,
            json!({
                "chain_id": 1,
                "token": {"address": address},
                "icon_url": null,
                "clear": true,
            })
        );
        assert_eq!(fx.served_icon(1, &ERC20).await, None);

        // Only the two touched `tokens` rows changed; no asset did.
        let rows = fx.token_rows().await;
        let erc20 = rows.iter().find(|row| row.chain_id == 1).unwrap();
        assert_eq!(erc20.token_icon, None);
        let native = rows.iter().find(|row| row.chain_id == 100).unwrap();
        assert_eq!(native.token_icon.as_deref(), Some(GOOD_URL));
        assert_eq!(native.address, native_before.address);
        assert_eq!(native.r#type, native_before.r#type);
        assert_eq!(
            fx.asset_tables().await,
            assets_before,
            "stats_assets and stats_asset_tokens are not part of this method"
        );
    }

    #[tokio::test]
    #[ignore = "needs database"]
    async fn admin_handler_token_icon_unknown_token_is_not_found_without_audit() {
        let fx = fixture("admin_handler_token_icon_not_found").await;
        let tokens_before = fx.token_rows().await;
        let unknown_address = format!("0x{}", "cd".repeat(20));
        let erc20 = erc20_address();

        let requests = [
            (
                "unknown address",
                token_request(1, Some(&unknown_address), None, Some(GOOD_URL), None, "r"),
            ),
            (
                "known address on another chain",
                token_request(100, Some(&erc20), None, Some(GOOD_URL), None, "r"),
            ),
            (
                "native token of a chain without a row",
                token_request(1, None, Some(true), Some(GOOD_URL), None, "r"),
            ),
            (
                "unknown chain",
                token_request(424242, Some(&erc20), None, Some(GOOD_URL), None, "r"),
            ),
        ];
        for (label, request) in requests {
            let status = fx.call_token(request).await.expect_err(label);
            assert_eq!(status.code(), Code::NotFound, "{label}");
            assert!(
                status.message().contains("token row"),
                "{label}: {}",
                status.message()
            );
        }

        assert_eq!(fx.audit_log().await.len(), 0);
        assert_eq!(fx.token_rows().await, tokens_before, "nothing was inserted");
    }

    #[tokio::test]
    #[ignore = "needs database"]
    async fn admin_handler_token_icon_validation_errors_write_no_audit() {
        let fx = fixture("admin_handler_token_icon_validation").await;
        let tokens_before = fx.token_rows().await;
        let erc20 = erc20_address();
        let zero_address = format!("0x{}", "00".repeat(20));

        let requests = [
            (
                "no selector",
                token_request(1, None, None, Some(GOOD_URL), None, "r"),
            ),
            (
                "both selectors",
                token_request(1, Some(&erc20), Some(true), Some(GOOD_URL), None, "r"),
            ),
            (
                "native=false",
                token_request(1, None, Some(false), Some(GOOD_URL), None, "r"),
            ),
            (
                "zero address",
                token_request(1, Some(&zero_address), None, Some(GOOD_URL), None, "r"),
            ),
            (
                "short address",
                token_request(1, Some("0xabcd"), None, Some(GOOD_URL), None, "r"),
            ),
            (
                "address without 0x",
                token_request(1, Some(&erc20[2..]), None, Some(GOOD_URL), None, "r"),
            ),
            (
                "http url",
                token_request(
                    1,
                    Some(&erc20),
                    None,
                    Some("http://example.com/i.png"),
                    None,
                    "r",
                ),
            ),
            (
                "both icon fields",
                token_request(1, Some(&erc20), None, Some(GOOD_URL), Some(true), "r"),
            ),
            (
                "clear=false",
                token_request(1, Some(&erc20), None, None, Some(false), "r"),
            ),
            (
                "neither icon field",
                token_request(1, Some(&erc20), None, None, None, "r"),
            ),
            (
                "empty reason",
                token_request(1, Some(&erc20), None, Some(GOOD_URL), None, ""),
            ),
            (
                "blank reason",
                token_request(1, Some(&erc20), None, Some(GOOD_URL), None, "  \n "),
            ),
        ];
        for (label, request) in requests {
            let status = fx.call_token(request).await.expect_err(label);
            assert_eq!(status.code(), Code::InvalidArgument, "{label}");
        }

        assert_eq!(fx.audit_log().await.len(), 0);
        assert_eq!(fx.token_rows().await, tokens_before);
    }
}
