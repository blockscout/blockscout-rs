use crate::{repository::user_op::user_ops_blocks_rel, types::paymaster::Paymaster};
use entity::user_operations::{Column, Entity};
use ethers::prelude::Address;
use sea_orm::{
    prelude::Expr, sea_query::IntoCondition, ColumnTrait, DatabaseConnection, EntityTrait,
    FromQueryResult, IntoSimpleExpr, JoinType, QueryFilter, QueryOrder, QuerySelect,
};

#[derive(FromQueryResult, Clone)]
pub struct PaymasterDB {
    pub paymaster: Vec<u8>,
    pub total_ops: i64,
}

pub async fn find_paymaster_by_address(
    db: &DatabaseConnection,
    addr: Address,
) -> Result<Option<Paymaster>, anyhow::Error> {
    let paymaster = Entity::find()
        .select_only()
        .column(Column::Paymaster)
        .column_as(Column::Paymaster.count(), "total_ops")
        .join_rev(JoinType::Join, user_ops_blocks_rel())
        .filter(Column::Paymaster.eq(addr.as_bytes()).into_condition())
        .group_by(Column::Paymaster)
        .into_model::<PaymasterDB>()
        .one(db)
        .await?
        .map(Paymaster::from);

    Ok(paymaster)
}

pub async fn list_paymasters(
    db: &DatabaseConnection,
    page_token: Option<(u64, Address)>,
    limit: u64,
) -> Result<(Vec<Paymaster>, Option<(u64, Address)>), anyhow::Error> {
    let page_token = page_token.unwrap_or((i64::MAX as u64, Address::zero()));

    let paymasters: Vec<Paymaster> = Entity::find()
        .select_only()
        .column(Column::Paymaster)
        .column_as(Column::Paymaster.count(), "total_ops")
        .join_rev(JoinType::Join, user_ops_blocks_rel())
        .filter(Column::Paymaster.is_not_null().into_condition())
        .group_by(Column::Paymaster)
        .having(
            Expr::tuple([
                Column::Paymaster.count(),
                Column::Paymaster.into_simple_expr(),
            ])
            .lte(Expr::tuple([
                page_token.0.into(),
                page_token.1.as_bytes().into(),
            ])),
        )
        .order_by_desc(Expr::cust("2"))
        .order_by_desc(Expr::cust("1"))
        .limit(limit + 1)
        .into_model::<PaymasterDB>()
        .all(db)
        .await?
        .into_iter()
        .map(Paymaster::from)
        .collect();

    match paymasters.get(limit as usize) {
        Some(a) => Ok((
            paymasters[0..limit as usize].to_vec(),
            Some((a.total_ops as u64, a.paymaster)),
        )),
        None => Ok((paymasters, None)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::repository::tests::get_shared_db;
    use pretty_assertions::assert_eq;

    #[tokio::test]
    async fn find_paymaster_by_address_ok() {
        let db = get_shared_db().await;

        let addr = Address::from_low_u64_be(0xe3);
        let item = find_paymaster_by_address(&db, addr).await.unwrap();
        assert_eq!(item, None);

        let addr = Address::from_low_u64_be(0xe1);
        let item = find_paymaster_by_address(&db, addr).await.unwrap();
        assert_eq!(
            item,
            Some(Paymaster {
                paymaster: addr,
                total_ops: 10,
            })
        );
    }

    #[tokio::test]
    async fn list_paymasters_ok() {
        let db = get_shared_db().await;

        let (items, next_page_token) = list_paymasters(&db, None, 1).await.unwrap();
        assert_eq!(items.len(), 1);
        assert_ne!(next_page_token, None);

        let (items, next_page_token) = list_paymasters(&db, next_page_token, 1).await.unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(next_page_token, None);
        assert!(items.iter().all(|a| a.total_ops == 10))
    }
}
