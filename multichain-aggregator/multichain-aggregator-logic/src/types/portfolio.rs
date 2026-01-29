use super::address_token_balances::fiat_balance_query;
use crate::{proto, types::ChainId};
use entity::address_token_balances::Entity;
use sea_orm::{
    DerivePartialModel,
    prelude::{BigDecimal, Expr},
    sea_query::SimpleExpr,
};

pub fn portfolio_fiat_balance_sum_query() -> SimpleExpr {
    Expr::cust_with_expr("COALESCE(SUM($1), 0)", fiat_balance_query())
}

#[derive(DerivePartialModel, Clone, Debug)]
#[sea_orm(entity = "Entity", from_query_result)]
pub struct PortfolioChainValue {
    pub chain_id: ChainId,
    #[sea_orm(from_expr = r#"portfolio_fiat_balance_sum_query()"#)]
    pub value: BigDecimal,
}

#[derive(Debug, Clone)]
pub struct AddressPortfolio {
    pub total_value: BigDecimal,
    pub chain_values: Vec<PortfolioChainValue>,
}

impl From<AddressPortfolio> for proto::AddressPortfolio {
    fn from(v: AddressPortfolio) -> Self {
        let chain_values = v
            .chain_values
            .into_iter()
            .map(|c| (c.chain_id.to_string(), c.value.to_plain_string()))
            .collect();
        Self {
            total_value: v.total_value.to_plain_string(),
            chain_values,
        }
    }
}
