use ethers::prelude::Address;
use sea_orm::{
    prelude::Expr, sea_query::IntoCondition, ColumnTrait, DatabaseConnection, EntityTrait,
    FromQueryResult, IntoSimpleExpr, JoinType, QueryFilter, QueryOrder, QuerySelect,
};

use entity::user_operations::{Column, Entity};

use crate::{repository::user_op::user_ops_blocks_rel, types::factory::Factory};

#[derive(FromQueryResult, Clone)]
pub struct FactoryDB {
    pub factory: Vec<u8>,
    pub total_accounts: i64,
}

pub async fn find_factory_by_address(
    db: &DatabaseConnection,
    addr: Address,
) -> Result<Option<Factory>, anyhow::Error> {
    let factory = Entity::find()
        .select_only()
        .column(Column::Factory)
        .column_as(Column::Factory.count(), "total_accounts")
        .join_rev(JoinType::Join, user_ops_blocks_rel())
        .filter(Column::Factory.eq(addr.as_bytes()).into_condition())
        .group_by(Column::Factory)
        .into_model::<FactoryDB>()
        .one(db)
        .await?
        .map(Factory::from);

    Ok(factory)
}

pub async fn list_factories(
    db: &DatabaseConnection,
    page_token: Option<(u64, Address)>,
    limit: u64,
) -> Result<(Vec<Factory>, Option<(u64, Address)>), anyhow::Error> {
    let page_token = page_token.unwrap_or((i64::MAX as u64, Address::zero()));

    let factories: Vec<Factory> = Entity::find()
        .select_only()
        .column(Column::Factory)
        .column_as(Column::Factory.count(), "total_accounts")
        .join_rev(JoinType::Join, user_ops_blocks_rel())
        .filter(Column::Factory.is_not_null().into_condition())
        .group_by(Column::Factory)
        .having(
            Expr::tuple([Column::Factory.count(), Column::Factory.into_simple_expr()]).lte(
                Expr::tuple([page_token.0.into(), page_token.1.as_bytes().into()]),
            ),
        )
        .order_by_desc(Expr::cust("2"))
        .order_by_desc(Expr::cust("1"))
        .limit(limit + 1)
        .into_model::<FactoryDB>()
        .all(db)
        .await?
        .into_iter()
        .map(Factory::from)
        .collect();

    match factories.get(limit as usize) {
        Some(a) => Ok((
            factories[0..limit as usize].to_vec(),
            Some((a.total_accounts as u64, a.factory)),
        )),
        None => Ok((factories, None)),
    }
}
