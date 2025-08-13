use crate::{
    repository::{
        macros::{col, expr_as, map_col},
        tokens::{aggregated_tokens_query, normal_tokens_query},
    },
    types::{
        ChainId,
        address_token_balances::{
            AddressTokenBalance, AggregatedAddressTokenBalance, ExtendedAddressTokenBalance,
            chain_values_expr,
        },
    },
};
use bigdecimal::BigDecimal;
use entity::{
    address_token_balances::{ActiveModel, Column, Entity},
    sea_orm_active_enums::TokenType,
    tokens,
};
use sea_orm::{
    ColumnTrait, ConnectionTrait, DbErr, EntityTrait, FromQueryResult, JoinType, Order,
    PartialModelTrait, QueryFilter, QuerySelect, QueryTrait,
    prelude::Expr,
    sea_query::{
        self, ColumnRef, CommonTableExpression, IntoColumnRef, IntoIden, Keyword, NullOrdering,
        OnConflict, Query, SelectExpr, SelectStatement, SimpleExpr, WithClause,
    },
};

pub async fn upsert_many<C>(
    db: &C,
    mut address_token_balances: Vec<AddressTokenBalance>,
) -> Result<(), DbErr>
where
    C: ConnectionTrait,
{
    address_token_balances.sort_by(|a, b| {
        (
            &a.address_hash,
            &a.chain_id,
            &a.token_address_hash,
            &a.token_id,
        )
            .cmp(&(
                &b.address_hash,
                &b.chain_id,
                &b.token_address_hash,
                &b.token_id,
            ))
    });
    let address_token_balances = address_token_balances.into_iter().map(ActiveModel::from);

    Entity::insert_many(address_token_balances)
        .on_conflict(
            OnConflict::new()
                .exprs([
                    Expr::col(Column::AddressHash),
                    Expr::col(Column::ChainId),
                    Expr::col(Column::TokenAddressHash),
                    Expr::expr(Expr::cust_with_expr(
                        "COALESCE($1, -1)",
                        Column::TokenId.into_expr(),
                    )),
                ])
                .update_columns([Column::Value])
                .value(Column::UpdatedAt, Expr::current_timestamp())
                .to_owned(),
        )
        .do_nothing()
        .exec_without_returning(db)
        .await?;

    Ok(())
}

pub type ListAddressTokensPageToken = (Option<BigDecimal>, BigDecimal, i64);

fn prepare_list_by_address_query(
    address: alloy_primitives::Address,
    token_types: Vec<TokenType>,
    chain_ids: Vec<i64>,
    page_size: u64,
    page_token: Option<ListAddressTokensPageToken>,
) -> SelectStatement {
    let tokens_rel = Entity::belongs_to(tokens::Entity)
        .from((Column::TokenAddressHash, Column::ChainId))
        .to((tokens::Column::AddressHash, tokens::Column::ChainId))
        .into();

    let base_query = Entity::find()
        .join(JoinType::InnerJoin, tokens_rel)
        .filter(Column::AddressHash.eq(address.as_slice()))
        .filter(Column::ChainId.is_in(chain_ids))
        .filter(Column::Value.gt(0))
        .apply_if(
            (!token_types.is_empty()).then_some(token_types),
            |q, token_types| q.filter(tokens::Column::TokenType.is_in(token_types)),
        );

    let base_query = ExtendedAddressTokenBalance::select_cols(base_query.select_only())
        .as_query()
        .to_owned();

    let base_cte = CommonTableExpression::new()
        .query(base_query)
        .table_name("base")
        .to_owned();

    let mut normal_tokens_query = normal_tokens_query("base")
        // Extend base query with `AddressTokenBalances` columns
        .exprs([
            col!("id"),
            expr_as!(Expr::val(address.as_slice()), "address_hash"),
            col!("value"),
            expr_as!(
                Expr::cust_with_expr("jsonb_build_array($1)", chain_values_expr()),
                "chain_values"
            ),
            col!("token_id"),
            col!("fiat_balance"),
        ])
        .to_owned();

    let aggregated_tokens_query = aggregated_tokens_query("base")
        // Extend base query with `AddressTokenBalances` columns
        .exprs([
            map_col!("MIN($1)", "id"),
            expr_as!(Expr::val(address.as_slice()), "address_hash"),
            map_col!("SUM($1)", "value"),
            expr_as!(
                Expr::cust_with_expr("jsonb_agg($1)", chain_values_expr()),
                "chain_values"
            ),
            expr_as!(Keyword::Null, "token_id"),
            map_col!("AVG($1)", "fiat_balance"),
        ])
        .to_owned();

    let union_cte = CommonTableExpression::new()
        .query(
            normal_tokens_query
                .union(sea_query::UnionType::All, aggregated_tokens_query)
                .to_owned(),
        )
        .table_name("tokens")
        .to_owned();

    let apply_pagination = move |q: &mut SelectStatement| {
        q.apply_if(page_token, |q, page_token| {
            let (fiat_value, value, id) = page_token;
            // Handle pagination similar to how it's done in the Elixir backend
            // https://github.com/blockscout/blockscout/blob/dff7814bb06327a9f80d0850470e8798e48301fe/apps/explorer/lib/explorer/chain.ex#L2882-L2917
            match fiat_value {
                None => q.and_where(Expr::cust_with_exprs(
                    "$1 IS NULL AND ($2 < $3 OR ($2 = $3 AND $4 < $5))",
                    [
                        Expr::col("fiat_balance").into(),
                        Expr::col("value").into(),
                        Expr::value(value),
                        Expr::col("id").into(),
                        Expr::value(id),
                    ],
                )),
                Some(fiat_value) => q.and_where(Expr::cust_with_exprs(
                    "$1 < $2 OR $1 IS NULL OR ($1 = $2 AND ($3 < $4 OR ($3 = $4 AND $5 < $6)))",
                    [
                        Expr::col("fiat_balance").into(),
                        Expr::value(fiat_value),
                        Expr::col("value").into(),
                        Expr::value(value),
                        Expr::col("id").into(),
                        Expr::value(id),
                    ],
                )),
            };
        })
        .order_by_with_nulls("fiat_balance", Order::Desc, NullOrdering::Last)
        .order_by(Column::Value, Order::Desc)
        .order_by(Column::Id, Order::Desc)
        .limit(page_size + 1)
        .to_owned()
    };

    let mut query = SelectStatement::new()
        .with_cte(WithClause::new().cte(base_cte).cte(union_cte).to_owned())
        .column(ColumnRef::Asterisk)
        .from("tokens")
        .to_owned();

    apply_pagination(&mut query)
}

pub async fn list_by_address<C>(
    db: &C,
    address: alloy_primitives::Address,
    token_types: Vec<TokenType>,
    chain_ids: Vec<i64>,
    page_size: u64,
    page_token: Option<ListAddressTokensPageToken>,
) -> Result<
    (
        Vec<AggregatedAddressTokenBalance>,
        Option<ListAddressTokensPageToken>,
    ),
    DbErr,
>
where
    C: ConnectionTrait,
{
    let query =
        prepare_list_by_address_query(address, token_types, chain_ids, page_size, page_token);

    let balances =
        AggregatedAddressTokenBalance::find_by_statement(db.get_database_backend().build(&query))
            .all(db)
            .await?;

    if balances.len() as u64 > page_size {
        Ok((
            balances[..page_size as usize].to_vec(),
            balances
                .get(page_size as usize - 1)
                .map(|a| (a.fiat_balance.clone(), a.value.clone(), a.id)),
        ))
    } else {
        Ok((balances, None))
    }
}

pub async fn check_if_tokens_at_address<C>(
    db: &C,
    address: alloy_primitives::Address,
    cluster_chain_ids: Vec<ChainId>,
) -> Result<bool, DbErr>
where
    C: ConnectionTrait,
{
    let query = Query::select()
        .expr(Expr::exists(
            Query::select()
                .column(Column::Id)
                .from(Entity)
                .and_where(Column::AddressHash.eq(address.as_slice()))
                .and_where(Column::ChainId.is_in(cluster_chain_ids))
                .to_owned(),
        ))
        .to_owned();

    db.query_one(db.get_database_backend().build(&query))
        .await?
        .expect("expr should be present")
        .try_get_by_index(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use sea_orm::sea_query::PostgresQueryBuilder;

    pub fn normalize_sql(statement: &str) -> String {
        statement.split_whitespace().collect::<Vec<_>>().join(" ")
    }

    #[tokio::test]
    async fn statements_are_correct() {
        let address = "0x0000000000000000000000000000000000000000"
            .parse()
            .unwrap();
        let token_types = vec![TokenType::Erc7802];
        let chain_ids = vec![1, 2, 3];
        let page_size = 10;
        let page_token = None;

        let query =
            prepare_list_by_address_query(address, token_types, chain_ids, page_size, page_token);

        let sql = query.to_string(PostgresQueryBuilder);
        let expected = r#"
            WITH "base" AS (SELECT
                    "address_token_balances"."id" AS "id",
                    "address_token_balances"."address_hash" AS "address_hash",
                    "address_token_balances"."value" AS "value",
                    "address_token_balances"."token_id" AS "token_id",
                    "tokens"."address_hash" AS "token_address_hash",
                    "tokens"."chain_id" AS "token_chain_id",
                    "tokens"."name" AS "token_name",
                    "tokens"."symbol" AS "token_symbol",
                    "tokens"."decimals" AS "token_decimals",
                    CAST("tokens"."token_type" AS "text") AS "token_token_type",
                    "tokens"."icon_url" AS "token_icon_url",
                    "tokens"."fiat_value" AS "token_fiat_value",
                    "tokens"."circulating_market_cap" AS "token_circulating_market_cap",
                    "tokens"."total_supply" AS "token_total_supply",
                    "tokens"."holders_count" AS "token_holders_count",
                    "tokens"."transfers_count" AS "token_transfers_count",
                    ("address_token_balances"."value" * "tokens"."fiat_value") / (POWER(10,"tokens"."decimals")::numeric) AS "fiat_balance"
                FROM "address_token_balances"
                    INNER JOIN "tokens" ON "address_token_balances"."token_address_hash" = "tokens"."address_hash"
                    AND "address_token_balances"."chain_id" = "tokens"."chain_id"
                WHERE "address_token_balances"."address_hash" = '\x0000000000000000000000000000000000000000'
                    AND "address_token_balances"."chain_id" IN (1, 2, 3)
                    AND "address_token_balances"."value" > 0
                    AND "tokens"."token_type" IN (CAST('ERC-7802' AS "token_type"))) ,
            "tokens" AS (SELECT
                    "token_address_hash",
                    "token_name",
                    "token_symbol",
                    "token_decimals",
                    "token_token_type",
                    "token_icon_url",
                    "token_fiat_value",
                    "token_circulating_market_cap",
                    "token_total_supply",
                    "token_holders_count",
                    "token_transfers_count",
                    jsonb_build_array(jsonb_build_object('chain_id',"token_chain_id",'holders_count',"token_holders_count",'total_supply',"token_total_supply")) AS "token_chain_infos",
                    "id",
                    '\x0000000000000000000000000000000000000000' AS "address_hash",
                    "value",
                    jsonb_build_array(jsonb_build_object('chain_id',"token_chain_id",'value',"value")) AS "chain_values",
                    "token_id",
                    "fiat_balance"
                FROM "base"
                WHERE "token_token_type" <> 'ERC-7802'
                UNION ALL
                (SELECT "token_address_hash",
                    "token_name",
                    "token_symbol",
                    "token_decimals",
                    'ERC-7802' AS "token_token_type",
                    MAX("token_icon_url") AS "token_icon_url",
                    AVG("token_fiat_value") AS "token_fiat_value",
                    AVG("token_circulating_market_cap") AS "token_circulating_market_cap",
                    SUM("token_total_supply") AS "token_total_supply",
                    SUM("token_holders_count") AS "token_holders_count",
                    SUM("token_transfers_count") AS "token_transfers_count",
                    jsonb_agg(jsonb_build_object('chain_id',"token_chain_id",'holders_count',"token_holders_count",'total_supply',"token_total_supply")) AS "token_chain_infos",
                    MIN("id") AS "id",
                    '\x0000000000000000000000000000000000000000' AS "address_hash",
                    SUM("value") AS "value",
                    jsonb_agg(jsonb_build_object('chain_id',"token_chain_id",'value',"value")) AS "chain_values",
                    NULL AS "token_id",
                    AVG("fiat_balance") AS "fiat_balance"
                    FROM "base"
                    WHERE "token_token_type" = 'ERC-7802'
                    GROUP BY 
                        "token_address_hash",
                        "token_name",
                        "token_symbol",
                        "token_decimals"))
            SELECT *
            FROM "tokens"
            ORDER BY "fiat_balance" DESC NULLS LAST,
            "value" DESC,
            "id" DESC
            LIMIT 11
            "#;

        assert_eq!(normalize_sql(expected), normalize_sql(&sql));
    }
}
