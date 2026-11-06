// SPDX-License-Identifier: LicenseRef-Blockscout

use super::scope::ReadScope;
use crate::proto::Pagination;
use chrono::NaiveDateTime;
use interchain_indexer_logic::{
    ChainBridgeFilter, IndexedChains, ListSubject,
    pagination::{ListMarker, OutputPagination},
    protocol_metadata,
};
use sea_orm::JsonValue;
use tonic::Status;

/// Delegates to `interchain_indexer_logic::protocol_metadata::rfc3339_millis_z`
/// so the Read API's general timestamp rendering and the
/// `multiple_executions` namespace's `additional_executions[].timestamp`
/// (assembled in logic, not here) can never drift into two different formats.
pub fn db_datetime_to_string(ts: NaiveDateTime) -> String {
    protocol_metadata::rfc3339_millis_z(ts)
}

pub fn map_db_error(err: anyhow::Error) -> tonic::Status {
    tracing::error!("database error: {:?}", err);
    tonic::Status::internal("internal server error")
}

pub fn sort_json_value(value: JsonValue) -> JsonValue {
    match value {
        JsonValue::Object(map) => {
            let mut entries: Vec<_> = map.into_iter().collect();
            entries.sort_by(|(left, _), (right, _)| left.cmp(right));
            let mut sorted = serde_json::Map::new();
            for (key, value) in entries {
                sorted.insert(key, sort_json_value(value));
            }
            JsonValue::Object(sorted)
        }
        JsonValue::Array(values) => {
            JsonValue::Array(values.into_iter().map(sort_json_value).collect())
        }
        other => other,
    }
}

pub fn parse_chain_ids_csv(param: &str, input: Option<&str>) -> Result<Vec<i64>, Status> {
    let Some(input) = input.map(str::trim) else {
        return Ok(Vec::new());
    };
    if input.is_empty() {
        return Ok(Vec::new());
    }

    input
        .split(',')
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .map(|part| {
            part.parse::<i64>().map_err(|_| {
                Status::invalid_argument(format!(
                    "invalid {param} value `{part}`: expected comma-separated int64 ids"
                ))
            })
        })
        .collect()
}

pub fn parse_bridge_ids_csv(input: Option<&str>) -> Result<Vec<i32>, Status> {
    let Some(input) = input.map(str::trim) else {
        return Ok(Vec::new());
    };
    if input.is_empty() {
        return Ok(Vec::new());
    }

    input
        .split(',')
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .map(|part| {
            part.parse::<i32>().map_err(|_| {
                Status::invalid_argument(format!(
                    "invalid bridge_ids value `{part}`: expected comma-separated int32 ids"
                ))
            })
        })
        .collect()
}

/// Plain-list `q`: blank → `All`; optional `0x`/`0X` prefix then 40 hex digits
/// → `Address`, or 64 hex digits → `Hash` (any letter case); anything else →
/// `None` (no row can match, so the caller serves an empty page without
/// querying).
pub fn parse_list_q(q: Option<&str>) -> Option<ListSubject> {
    let q = q.unwrap_or_default().trim();
    let digits = q
        .strip_prefix("0x")
        .or_else(|| q.strip_prefix("0X"))
        .unwrap_or(q);
    match hex::decode(digits) {
        _ if q.is_empty() => Some(ListSubject::All),
        Ok(bytes) if bytes.len() == 20 => Some(ListSubject::Address(bytes)),
        Ok(bytes) if bytes.len() == 32 => Some(ListSubject::Hash(bytes)),
        _ => None,
    }
}

pub fn non_empty<T>(v: Vec<T>) -> Option<Vec<T>> {
    if v.is_empty() { None } else { Some(v) }
}

/// Builds the shared `ChainBridgeFilter` for every filtered list/counter
/// endpoint. The already-parsed [`ReadScope`] carries home, counterparty,
/// bridges and the unindexed opt-in; this function parses only the directional
/// `src_chain_ids` / `dst_chain_ids` lists (empty-to-`None` normalization
/// included), so a new filter dimension is maintained in one place. Malformed
/// values return labeled `InvalidArgument`.
///
/// `indexed_chains` and the scope's `include_unindexed_chains` add the sixth
/// dimension: by default (`include_unindexed_chains = false`) results are
/// restricted to rows their own bridge could have fully observed (see
/// [`ChainBridgeFilter::only_indexed_by_bridge`]); `include_unindexed_chains =
/// true` or an `AllIndexed` configuration disables the restriction entirely.
pub fn build_chain_bridge_filter(
    scope: &ReadScope,
    src_chain_ids: Option<&str>,
    dst_chain_ids: Option<&str>,
    indexed_chains: &IndexedChains,
) -> Result<ChainBridgeFilter, Status> {
    let only_indexed_by_bridge = (!scope.include_unindexed_chains)
        .then(|| indexed_chains.configured_pairs(scope.bridge_ids.as_deref()))
        .flatten();

    Ok(ChainBridgeFilter {
        home_chain_id: scope.home_chain_id,
        counterparty_chain_ids: scope.counterparty_chain_ids.clone(),
        src_chain_ids: non_empty(parse_chain_ids_csv("src_chain_ids", src_chain_ids)?),
        dst_chain_ids: non_empty(parse_chain_ids_csv("dst_chain_ids", dst_chain_ids)?),
        bridge_ids: scope.bridge_ids.clone(),
        only_indexed_by_bridge,
    })
}

/// Checked conversion of an optional request `bridge_id` (`u32`) to the storage
/// `i32`. Values above `i32::MAX` are client input errors and are rejected with
/// `InvalidArgument` rather than silently wrapping via `as`.
pub fn checked_bridge_id(bridge_id: Option<u32>) -> Result<Option<i32>, Status> {
    bridge_id
        .map(i32::try_from)
        .transpose()
        .map_err(|_| Status::invalid_argument("bridge_id exceeds the supported int32 range"))
}

/// Decodes an optional request `page_token`. A malformed token is client
/// input (`InvalidArgument`), never an internal error, and is not logged.
pub fn parse_page_token<M: ListMarker>(page_token: Option<&str>) -> Result<Option<M>, Status> {
    page_token
        .map(M::from_token)
        .transpose()
        .map_err(|e| Status::invalid_argument(format!("invalid page_token: {e}")))
}

/// Renders both sides of an `OutputPagination` into the shared response
/// cursor. Infallible by design: a side whose marker cannot be encoded is
/// logged here (the handling point) and omitted, so the page itself is still
/// served. No marker a list query builds is expected to fail: bridged-tokens
/// bounds its name sort key to fit the 512-byte name cap
/// (`bridged_tokens_query.rs`). The omission is the safety net if one ever
/// does.
pub fn page_params<M: ListMarker + std::fmt::Debug>(
    pagination: OutputPagination<M>,
) -> (Option<Pagination>, Option<Pagination>) {
    (
        encode_page_token(pagination.next_marker),
        encode_page_token(pagination.prev_marker),
    )
}

fn encode_page_token<M: ListMarker + std::fmt::Debug>(marker: Option<M>) -> Option<Pagination> {
    let marker = marker?;
    marker
        .token()
        .inspect_err(|e| tracing::error!(err = ?e, marker = ?marker, "failed to encode page token"))
        .ok()
        .map(|page_token| Pagination { page_token })
}

/// Parses the pagination fields every list request shares by name
/// (`page_token`, `page_size`, `last_page`) and returns
/// `(inner, input_pagination, page_size, is_last_page)`. A macro, not a fn:
/// the request types share these field names but no trait.
macro_rules! list_page_params {
    ($service:expr, $request:expr, $marker:ty) => {{
        let inner = $request.into_inner();
        let input_pagination =
            $crate::services::utils::parse_page_token::<$marker>(inner.page_token.as_deref())?;
        let page_size = inner
            .page_size
            .unwrap_or($service.api_settings.default_page_size)
            .clamp(1, $service.api_settings.max_page_size.get()) as usize;
        let is_last_page = inner.last_page.unwrap_or(false);
        Ok::<_, tonic::Status>((inner, input_pagination, page_size, is_last_page))
    }};
}
pub(crate) use list_page_params;

#[cfg(test)]
mod tests {
    use super::{
        build_chain_bridge_filter, checked_bridge_id, db_datetime_to_string, non_empty,
        page_params, parse_bridge_ids_csv, parse_chain_ids_csv, parse_list_q, parse_page_token,
    };
    use crate::services::scope::ReadScope;
    use interchain_indexer_logic::{
        IndexedChains, ListSubject,
        pagination::{
            BridgedTokensPaginationLogic, ListMarker, MessagesPaginationLogic, OutputPagination,
            PaginationDirection, StatsChainsPaginationLogic,
        },
    };

    fn messages_marker(direction: PaginationDirection, message_id: u64) -> MessagesPaginationLogic {
        MessagesPaginationLogic {
            timestamp: chrono::DateTime::from_timestamp(1_767_313_800, 0)
                .expect("valid timestamp")
                .naive_utc(),
            message_id,
            bridge_id: 1,
            direction,
        }
    }

    fn bridged_tokens_marker(name_sort: String) -> BridgedTokensPaginationLogic {
        BridgedTokensPaginationLogic {
            direction: PaginationDirection::Next,
            stats_asset_id: 42,
            name_blank: false,
            name_sort,
            count: 7,
        }
    }

    #[test]
    fn parse_page_token_absent_is_none() {
        assert_eq!(
            parse_page_token::<MessagesPaginationLogic>(None).unwrap(),
            None
        );
    }

    #[test]
    fn parse_page_token_rejects_malformed_as_invalid_argument() {
        let err = parse_page_token::<MessagesPaginationLogic>(Some("not*a*token"))
            .expect_err("must reject malformed token");
        assert_eq!(err.code(), tonic::Code::InvalidArgument);
        assert!(err.message().contains("invalid page_token"));
    }

    #[test]
    fn parse_page_token_decodes_valid_token() {
        let parsed =
            parse_page_token::<StatsChainsPaginationLogic>(Some("AQAAAAAAAAAAMgAAAAAAAABk"))
                .unwrap();
        assert_eq!(
            parsed,
            Some(StatsChainsPaginationLogic {
                direction: PaginationDirection::Next,
                count: 50,
                chain_id: 100,
            })
        );
    }

    #[test]
    fn page_params_none_sides_stay_none() {
        let (next, prev) = page_params::<StatsChainsPaginationLogic>(OutputPagination::default());
        assert_eq!(next, None);
        assert_eq!(prev, None);
    }

    #[test]
    fn page_params_encodes_both_sides() {
        let next_marker = messages_marker(PaginationDirection::Next, 0x1234);
        let prev_marker = messages_marker(PaginationDirection::Prev, 0x1200);
        let (next, prev) = page_params(OutputPagination {
            next_marker: Some(next_marker),
            prev_marker: Some(prev_marker),
        });
        assert_eq!(
            next.expect("next side is encoded").page_token,
            next_marker.token().unwrap()
        );
        assert_eq!(
            prev.expect("prev side is encoded").page_token,
            prev_marker.token().unwrap()
        );
    }

    #[test]
    fn page_params_omits_only_the_side_that_fails_to_encode() {
        // 513 bytes is over the 512-byte bridged-tokens name cursor cap, so
        // `token()` errs for this marker.
        let next_marker = bridged_tokens_marker("x".repeat(513));
        let prev_marker = bridged_tokens_marker("usdc".to_string());
        assert!(next_marker.token().is_err());

        let (next, prev) = page_params(OutputPagination {
            next_marker: Some(next_marker),
            prev_marker: Some(prev_marker.clone()),
        });
        assert_eq!(next, None);
        assert_eq!(
            prev.expect("prev side is still encoded").page_token,
            prev_marker.token().unwrap()
        );
    }

    /// `db_datetime_to_string` delegates to
    /// `interchain_indexer_logic::protocol_metadata::rfc3339_millis_z`, so the
    /// general Read API timestamp format and the `multiple_executions`
    /// namespace's `additional_executions[].timestamp` (assembled in logic)
    /// can never drift into two different formats.
    #[test]
    fn db_datetime_to_string_matches_protocol_metadata_rfc3339_millis_z() {
        let ts = chrono::DateTime::from_timestamp(1_752_785_570, 0)
            .unwrap()
            .naive_utc();
        assert_eq!(
            db_datetime_to_string(ts),
            interchain_indexer_logic::protocol_metadata::rfc3339_millis_z(ts)
        );
        assert_eq!(db_datetime_to_string(ts), "2025-07-17T20:52:50.000Z");
    }

    #[test]
    fn parse_chain_ids_csv_accepts_missing_and_empty() {
        assert_eq!(
            parse_chain_ids_csv("chain_ids", None).unwrap(),
            Vec::<i64>::new()
        );
        assert_eq!(
            parse_chain_ids_csv("chain_ids", Some("")).unwrap(),
            Vec::<i64>::new()
        );
        assert_eq!(
            parse_chain_ids_csv("chain_ids", Some("   ")).unwrap(),
            Vec::<i64>::new()
        );
    }

    #[test]
    fn parse_chain_ids_csv_parses_comma_separated_ids() {
        assert_eq!(
            parse_chain_ids_csv("chain_ids", Some("123,456, 789")).unwrap(),
            vec![123, 456, 789]
        );
    }

    #[test]
    fn parse_chain_ids_csv_rejects_invalid_values_with_param_label() {
        let err = parse_chain_ids_csv("counterparty_chain_ids", Some("123,abc"))
            .expect_err("must reject invalid id");
        assert_eq!(err.code(), tonic::Code::InvalidArgument);
        assert!(
            err.message()
                .contains("invalid counterparty_chain_ids value `abc`")
        );
    }

    #[test]
    fn parse_list_q_cases() {
        let addr = Some(ListSubject::Address(vec![0xab; 20]));
        let hash = Some(ListSubject::Hash(vec![0xcd; 32]));
        let digits = |unit: &str, n: usize| unit.repeat(n);
        let cases: Vec<(Option<String>, Option<ListSubject>)> = vec![
            (None, Some(ListSubject::All)),
            (Some(String::new()), Some(ListSubject::All)),
            (Some("  \t ".to_string()), Some(ListSubject::All)),
            (Some(format!("0x{}", digits("ab", 20))), addr.clone()),
            (Some(format!("0X{}", digits("AB", 20))), addr.clone()),
            (Some(format!("  {}  ", digits("aB", 20))), addr),
            (Some(format!("0x{}", digits("cd", 32))), hash.clone()),
            (Some(digits("cd", 32)), hash),
            (Some("0x".to_string()), None),
            (Some(digits("a", 38)), None),
            (Some(digits("a", 39)), None),
            (Some(digits("a", 41)), None),
            (Some(digits("a", 63)), None),
            (Some(digits("a", 65)), None),
            (Some(digits("a", 66)), None),
            (Some(format!("0x0x{}", digits("ab", 20))), None),
            (Some(format!("0x{}", digits("zz", 20))), None),
            (Some("foo".to_string()), None),
        ];
        for (input, expected) in cases {
            assert_eq!(parse_list_q(input.as_deref()), expected, "input: {input:?}");
        }
    }

    #[test]
    fn parse_bridge_ids_csv_accepts_missing_and_empty() {
        assert_eq!(parse_bridge_ids_csv(None).unwrap(), Vec::<i32>::new());
        assert_eq!(parse_bridge_ids_csv(Some("")).unwrap(), Vec::<i32>::new());
        assert_eq!(
            parse_bridge_ids_csv(Some("   ")).unwrap(),
            Vec::<i32>::new()
        );
    }

    #[test]
    fn parse_bridge_ids_csv_parses_comma_separated_ids() {
        assert_eq!(parse_bridge_ids_csv(Some("1,2, 3")).unwrap(), vec![1, 2, 3]);
    }

    #[test]
    fn parse_bridge_ids_csv_rejects_invalid_values() {
        let err = parse_bridge_ids_csv(Some("1,abc")).expect_err("must reject invalid id");
        assert_eq!(err.code(), tonic::Code::InvalidArgument);
        assert!(err.message().contains("invalid bridge_ids value `abc`"));
    }

    #[test]
    fn parse_bridge_ids_csv_rejects_i32_overflow() {
        let err = parse_bridge_ids_csv(Some("3000000000")).expect_err("must reject i32 overflow");
        assert_eq!(err.code(), tonic::Code::InvalidArgument);
        assert!(
            err.message()
                .contains("invalid bridge_ids value `3000000000`")
        );
    }

    #[test]
    fn checked_bridge_id_passes_none_and_in_range() {
        assert_eq!(checked_bridge_id(None).unwrap(), None);
        assert_eq!(checked_bridge_id(Some(0)).unwrap(), Some(0));
        assert_eq!(checked_bridge_id(Some(1)).unwrap(), Some(1));
        assert_eq!(
            checked_bridge_id(Some(i32::MAX as u32)).unwrap(),
            Some(i32::MAX)
        );
    }

    #[test]
    fn checked_bridge_id_rejects_above_i32_max() {
        let err = checked_bridge_id(Some(i32::MAX as u32 + 1)).expect_err("must reject overflow");
        assert_eq!(err.code(), tonic::Code::InvalidArgument);
        assert!(
            err.message()
                .contains("bridge_id exceeds the supported int32 range")
        );
        let err = checked_bridge_id(Some(u32::MAX)).expect_err("must reject overflow");
        assert_eq!(err.code(), tonic::Code::InvalidArgument);
    }

    #[test]
    fn non_empty_maps_empty_to_none() {
        assert_eq!(non_empty(Vec::<i32>::new()), None);
        assert_eq!(non_empty(vec![1i32]), Some(vec![1]));
    }

    #[test]
    fn build_chain_bridge_filter_include_unindexed_true_clears_restriction() {
        let indexed = IndexedChains::from_pairs([(1, 1), (1, 100)]);
        let filter = build_chain_bridge_filter(
            &ReadScope {
                include_unindexed_chains: true,
                ..Default::default()
            },
            None,
            None,
            &indexed,
        )
        .unwrap();
        assert_eq!(filter.only_indexed_by_bridge, None);
    }

    #[test]
    fn build_chain_bridge_filter_default_sets_sorted_pairs() {
        let indexed = IndexedChains::from_pairs([(2, 250), (1, 100), (1, 1)]);
        let filter =
            build_chain_bridge_filter(&ReadScope::default(), None, None, &indexed).unwrap();
        assert_eq!(
            filter.only_indexed_by_bridge,
            Some(vec![(1, vec![1, 100]), (2, vec![250])])
        );
    }

    #[test]
    fn build_chain_bridge_filter_all_indexed_is_none_even_without_opt_in() {
        let filter = build_chain_bridge_filter(
            &ReadScope::default(),
            None,
            None,
            &IndexedChains::AllIndexed,
        )
        .unwrap();
        assert_eq!(filter.only_indexed_by_bridge, None);
    }

    #[test]
    fn build_chain_bridge_filter_prunes_pairs_to_requested_bridge_ids() {
        let indexed = IndexedChains::from_pairs([(1, 1), (2, 250), (3, 999)]);
        let filter = build_chain_bridge_filter(
            &ReadScope {
                bridge_ids: Some(vec![2]),
                ..Default::default()
            },
            None,
            None,
            &indexed,
        )
        .unwrap();
        assert_eq!(filter.bridge_ids, Some(vec![2]));
        assert_eq!(filter.only_indexed_by_bridge, Some(vec![(2, vec![250])]));
    }
}
