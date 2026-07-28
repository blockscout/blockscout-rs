// SPDX-License-Identifier: LicenseRef-Blockscout

use chrono::NaiveDateTime;
use interchain_indexer_logic::{ChainBridgeFilter, IndexedChains};
use sea_orm::JsonValue;
use tonic::Status;

pub fn db_datetime_to_string(ts: NaiveDateTime) -> String {
    ts.and_utc()
        .to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
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

pub fn non_empty<T>(v: Vec<T>) -> Option<Vec<T>> {
    if v.is_empty() { None } else { Some(v) }
}

/// Builds the shared `ChainBridgeFilter` from the CSV request fields common to
/// every filtered list/counter endpoint. Parsing and empty-to-`None`
/// normalization for all five CSV fields live here so a new filter dimension is
/// maintained in one place. Malformed values return labeled `InvalidArgument`.
///
/// `indexed_chains` and `include_unindexed` add the sixth dimension: by default
/// (`include_unindexed = false`) results are restricted to rows their own bridge
/// could have fully observed (see [`ChainBridgeFilter::only_indexed_by_bridge`]);
/// `include_unindexed = true` or an `AllIndexed` configuration disables the
/// restriction entirely.
pub fn build_chain_bridge_filter(
    home_chain_id: Option<i64>,
    counterparty_chain_ids: Option<&str>,
    src_chain_ids: Option<&str>,
    dst_chain_ids: Option<&str>,
    bridge_ids: Option<&str>,
    indexed_chains: &IndexedChains,
    include_unindexed: bool,
) -> Result<ChainBridgeFilter, Status> {
    let bridge_ids = non_empty(parse_bridge_ids_csv(bridge_ids)?);
    let only_indexed_by_bridge = (!include_unindexed)
        .then(|| indexed_chains.configured_pairs(bridge_ids.as_deref()))
        .flatten();

    Ok(ChainBridgeFilter {
        home_chain_id,
        counterparty_chain_ids: non_empty(parse_chain_ids_csv(
            "counterparty_chain_ids",
            counterparty_chain_ids,
        )?),
        src_chain_ids: non_empty(parse_chain_ids_csv("src_chain_ids", src_chain_ids)?),
        dst_chain_ids: non_empty(parse_chain_ids_csv("dst_chain_ids", dst_chain_ids)?),
        bridge_ids,
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

#[cfg(test)]
mod tests {
    use super::{
        build_chain_bridge_filter, checked_bridge_id, non_empty, parse_bridge_ids_csv,
        parse_chain_ids_csv,
    };
    use interchain_indexer_logic::IndexedChains;

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
        let filter =
            build_chain_bridge_filter(None, None, None, None, None, &indexed, true).unwrap();
        assert_eq!(filter.only_indexed_by_bridge, None);
    }

    #[test]
    fn build_chain_bridge_filter_default_sets_sorted_pairs() {
        let indexed = IndexedChains::from_pairs([(2, 250), (1, 100), (1, 1)]);
        let filter =
            build_chain_bridge_filter(None, None, None, None, None, &indexed, false).unwrap();
        assert_eq!(
            filter.only_indexed_by_bridge,
            Some(vec![(1, vec![1, 100]), (2, vec![250])])
        );
    }

    #[test]
    fn build_chain_bridge_filter_all_indexed_is_none_even_without_opt_in() {
        let filter = build_chain_bridge_filter(
            None,
            None,
            None,
            None,
            None,
            &IndexedChains::AllIndexed,
            false,
        )
        .unwrap();
        assert_eq!(filter.only_indexed_by_bridge, None);
    }

    #[test]
    fn build_chain_bridge_filter_prunes_pairs_to_requested_bridge_ids() {
        let indexed = IndexedChains::from_pairs([(1, 1), (2, 250), (3, 999)]);
        let filter =
            build_chain_bridge_filter(None, None, None, None, Some("2"), &indexed, false).unwrap();
        assert_eq!(filter.bridge_ids, Some(vec![2]));
        assert_eq!(filter.only_indexed_by_bridge, Some(vec![(2, vec![250])]));
    }
}
