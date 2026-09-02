// SPDX-License-Identifier: LicenseRef-Blockscout

use anyhow::anyhow;
use interchain_indexer_entity::bridges::Model as BridgeModel;
use interchain_indexer_logic::IndexedChains;
use sea_orm::ActiveEnum;
use tonic::Status;

use crate::proto::Bridge;

use super::utils::map_db_error;

/// Converts a `bridges` row plus the shared `IndexedChains` set into the
/// public `Bridge` directory entry.
///
/// `indexed_chain_ids` is read from `IndexedChains::chain_ids_for` (the
/// in-memory bridges config), never from `bridge_contracts`: a bridge removed
/// from config but still present in the `bridges` table (upserts never
/// delete rows) reports an empty list here rather than stale contract
/// history.
pub fn bridge_model_to_proto(
    model: BridgeModel,
    indexed_chains: &IndexedChains,
) -> Result<Bridge, Status> {
    let id =
        u32::try_from(model.id).map_err(|_| map_db_error(anyhow!("bridge id out of range")))?;
    // `chain_ids_for` sorts numerically; stringify only at the wire boundary so
    // the emitted order stays numeric rather than lexicographic.
    let indexed_chain_ids = indexed_chains
        .chain_ids_for(model.id)
        .into_iter()
        .map(|id| id.to_string())
        .collect();
    Ok(Bridge {
        id,
        name: model.name,
        r#type: model.r#type.map(|t| ActiveEnum::to_value(&t)),
        enabled: model.enabled,
        ui_url: model.ui_url,
        docs_url: model.docs_url,
        indexed_chain_ids,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use interchain_indexer_entity::sea_orm_active_enums::BridgeType;

    fn model(id: i32) -> BridgeModel {
        BridgeModel {
            id,
            name: "Test Bridge".to_string(),
            r#type: Some(BridgeType::Amb),
            enabled: true,
            ui_url: Some("https://example.com/ui".to_string()),
            docs_url: Some("https://example.com/docs".to_string()),
            api_url: None,
            created_at: Default::default(),
            updated_at: Default::default(),
        }
    }

    #[test]
    fn test_bridge_model_to_proto_no_configured_chains_is_empty() {
        // Bridge 1 absent from the map (removed from config, or never
        // configured): the directory reports an empty list, not the
        // permissive `may_observe` default.
        let indexed = IndexedChains::from_pairs([(2, 100)]);
        let bridge = bridge_model_to_proto(model(1), &indexed).unwrap();
        assert_eq!(bridge.indexed_chain_ids, Vec::<String>::new());
    }

    #[test]
    fn test_bridge_model_to_proto_multi_chain_is_sorted() {
        let indexed = IndexedChains::from_pairs([(1, 300), (1, 100), (1, 200)]);
        let bridge = bridge_model_to_proto(model(1), &indexed).unwrap();
        assert_eq!(bridge.indexed_chain_ids, vec!["100", "200", "300"]);
    }

    #[test]
    fn test_bridge_model_to_proto_preserves_other_fields() {
        let indexed = IndexedChains::from_pairs([(1, 1)]);
        let bridge = bridge_model_to_proto(model(1), &indexed).unwrap();
        assert_eq!(bridge.id, 1);
        assert_eq!(bridge.name, "Test Bridge");
        assert_eq!(bridge.r#type, Some("amb".to_string()));
        assert!(bridge.enabled);
        assert_eq!(bridge.ui_url, Some("https://example.com/ui".to_string()));
        assert_eq!(
            bridge.docs_url,
            Some("https://example.com/docs".to_string())
        );
    }

    /// Pins that the emitted order does not depend on the `HashSet`'s
    /// internal iteration order: many differently-shuffled insertion orders
    /// for the same chain-id set must all produce the same sorted output.
    #[test]
    fn test_bridge_model_to_proto_ordering_is_deterministic_across_insertion_orders() {
        let orderings: [[i64; 5]; 4] = [
            [500, 100, 300, 200, 400],
            [100, 200, 300, 400, 500],
            [400, 500, 100, 300, 200],
            [200, 400, 100, 500, 300],
        ];
        let expected = vec!["100", "200", "300", "400", "500"];

        for chains in orderings {
            let indexed = IndexedChains::from_bridges([(1, chains.to_vec())]);
            let bridge = bridge_model_to_proto(model(1), &indexed).unwrap();
            assert_eq!(
                bridge.indexed_chain_ids, expected,
                "insertion order {chains:?} must not change emitted order"
            );
        }
    }
}
