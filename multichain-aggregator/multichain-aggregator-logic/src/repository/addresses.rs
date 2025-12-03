use crate::{
    repository::{paginate_query, pagination::KeySpec, prepare_ts_query},
    types::{
        ChainId,
        addresses::{Address, AggregatedAddressInfo, ChainAddressInfo, StreamAddressUpdate},
    },
};
use alloy_primitives::Address as AddressAlloy;
use chrono::NaiveDateTime;
use entity::{
    address_coin_balances,
    addresses::{ActiveModel, Column, Entity},
};
use sea_orm::{
    ColumnTrait, ConnectionTrait, DbErr, EntityTrait, IntoSimpleExpr, JoinType, PartialModelTrait,
    QueryFilter, QuerySelect, QueryTrait, RelationDef, Select, prelude::Expr,
    sea_query::OnConflict,
};

pub async fn upsert_many<C>(db: &C, mut addresses: Vec<Address>) -> Result<(), DbErr>
where
    C: ConnectionTrait,
{
    addresses.sort_by(|a, b| (a.hash, a.chain_id).cmp(&(b.hash, b.chain_id)));
    let addresses = addresses.into_iter().map(ActiveModel::from);

    Entity::insert_many(addresses)
        .on_conflict(
            OnConflict::columns([Column::Hash, Column::ChainId])
                .update_columns([
                    Column::ContractName,
                    Column::IsContract,
                    Column::IsVerifiedContract,
                ])
                .value(Column::UpdatedAt, Expr::current_timestamp())
                .to_owned(),
        )
        .do_nothing()
        .exec_without_returning(db)
        .await?;

    Ok(())
}

fn coin_balances_rel() -> RelationDef {
    Entity::belongs_to(address_coin_balances::Entity)
        .from((Column::Hash, Column::ChainId))
        .to((
            address_coin_balances::Column::AddressHash,
            address_coin_balances::Column::ChainId,
        ))
        .into()
}

pub async fn get_aggregated_address_info<C>(
    db: &C,
    address: AddressAlloy,
    cluster_chain_ids: Option<Vec<ChainId>>,
) -> Result<Option<AggregatedAddressInfo>, DbErr>
where
    C: ConnectionTrait,
{
    let address_info = Entity::find()
        .join(JoinType::LeftJoin, coin_balances_rel())
        .filter(Column::Hash.eq(address.as_slice()))
        .apply_if(cluster_chain_ids, |q, cluster_chain_ids| {
            q.filter(Column::ChainId.is_in(cluster_chain_ids))
        })
        .group_by(Column::Hash)
        .into_partial_model::<AggregatedAddressInfo>()
        .one(db)
        .await?;

    Ok(address_info)
}

fn base_address_infos_query(
    addresses: Vec<AddressAlloy>,
    cluster_chain_ids: Option<Vec<ChainId>>,
    contract_name_query: Option<String>,
) -> Select<Entity> {
    Entity::find()
        .select_only()
        .join(JoinType::LeftJoin, coin_balances_rel())
        .apply_if(
            (!addresses.is_empty()).then_some(addresses),
            |q, addresses| {
                q.filter(
                    Column::Hash.is_in(
                        addresses
                            .into_iter()
                            .map(|a| a.to_vec())
                            .collect::<Vec<_>>(),
                    ),
                )
            },
        )
        .apply_if(cluster_chain_ids, |q, cluster_chain_ids| {
            q.filter(Column::ChainId.is_in(cluster_chain_ids))
        })
        .apply_if(contract_name_query, |q, query| {
            let ts_query = prepare_ts_query(&query);
            q.filter(Expr::cust_with_expr(
                "to_tsvector('english', contract_name) @@ to_tsquery($1)",
                ts_query,
            ))
        })
}

pub async fn list_aggregated_address_infos<C>(
    db: &C,
    addresses: Vec<AddressAlloy>,
    cluster_chain_ids: Option<Vec<ChainId>>,
    contract_name_query: Option<String>,
    page_size: u64,
    page_token: Option<AddressAlloy>,
) -> Result<(Vec<AggregatedAddressInfo>, Option<AddressAlloy>), DbErr>
where
    C: ConnectionTrait,
{
    let address_infos = AggregatedAddressInfo::select_cols(
        base_address_infos_query(addresses, cluster_chain_ids, contract_name_query)
            .group_by(Column::Hash),
    )
    .as_query()
    .to_owned();

    let order_keys = vec![KeySpec::asc(Expr::col(Column::Hash).into())];
    let page_token = page_token.map(|address| address.to_vec());

    paginate_query(
        db,
        address_infos,
        page_size,
        page_token,
        order_keys,
        |a: &AggregatedAddressInfo| *a.hash,
    )
    .await
}

pub async fn list_chain_address_infos<C>(
    db: &C,
    addresses: Vec<AddressAlloy>,
    cluster_chain_ids: Option<Vec<ChainId>>,
    contract_name_query: Option<String>,
    page_size: u64,
    page_token: Option<(AddressAlloy, ChainId)>,
) -> Result<(Vec<ChainAddressInfo>, Option<(AddressAlloy, ChainId)>), DbErr>
where
    C: ConnectionTrait,
{
    let address_infos = ChainAddressInfo::select_cols(base_address_infos_query(
        addresses,
        cluster_chain_ids,
        contract_name_query,
    ))
    .as_query()
    .to_owned();

    let order_keys = vec![
        KeySpec::asc(Column::Hash.into_simple_expr()),
        KeySpec::asc(Column::ChainId.into_simple_expr()),
    ];
    let page_token = page_token.map(|(address, chain_id)| (address.to_vec(), chain_id));

    paginate_query(
        db,
        address_infos,
        page_size,
        page_token,
        order_keys,
        |a: &ChainAddressInfo| (*a.hash, a.chain_info.chain_id),
    )
    .await
}

pub type StreamAddressUpdatesPageToken = (NaiveDateTime, AddressAlloy, ChainId);

pub async fn stream_address_updates<C>(
    db: &C,
    chain_ids: Vec<ChainId>,
    is_contract: Option<bool>,
    page_size: u64,
    page_token: Option<StreamAddressUpdatesPageToken>,
) -> Result<
    (
        Vec<StreamAddressUpdate>,
        Option<StreamAddressUpdatesPageToken>,
    ),
    DbErr,
>
where
    C: ConnectionTrait,
{
    let query = StreamAddressUpdate::select_cols(Entity::find().select_only())
        .apply_if(
            (!chain_ids.is_empty()).then_some(chain_ids),
            |q, chain_ids| q.filter(Column::ChainId.is_in(chain_ids)),
        )
        .apply_if(is_contract, |q, is_contract| {
            q.filter(Column::IsContract.eq(is_contract))
        })
        .as_query()
        .to_owned();

    let order_keys = vec![
        KeySpec::asc(Column::UpdatedAt.into_simple_expr()),
        KeySpec::asc(Column::Hash.into_simple_expr()),
        KeySpec::asc(Column::ChainId.into_simple_expr()),
    ];
    let page_token =
        page_token.map(|(updated_at, address, chain_id)| (updated_at, address.to_vec(), chain_id));

    paginate_query(
        db,
        query,
        page_size,
        page_token,
        order_keys,
        |a: &StreamAddressUpdate| (a.updated_at, *a.hash, a.chain_id),
    )
    .await
}
