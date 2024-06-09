use crate::{
    entity::subgraph::domain_event::DomainEventTransaction,
    protocols::{DomainName, Protocol},
    subgraphs_reader::{pagination::Order, sql::DbErr, EventSort, GetDomainHistoryInput},
};
use anyhow::Context;
use lazy_static::lazy_static;
use sqlx::postgres::PgPool;
use tera::Tera;
use tracing::instrument;

#[instrument(
    name = "find_transaction_events",
    skip(pool),
    err(level = "error"),
    level = "info"
)]
pub async fn find_transaction_events(
    pool: &PgPool,
    protocol: &Protocol,
    name: &DomainName,
    input: &GetDomainHistoryInput,
) -> Result<Vec<DomainEventTransaction>, DbErr> {
    let sort = input.sort;
    let order = input.order;
    let sql = sql_events_of_domain(&protocol.subgraph_schema, sort, order)
        .context("building sql from template")?;
    let transactions: Vec<DomainEventTransaction> =
        sqlx::query_as(&sql).bind(&name.id).fetch_all(pool).await?;
    Ok(transactions)
}

const SQL_HISTORY_TEMPLATE: &str = include_str!("history.sql");

lazy_static! {
    pub static ref TEMPLATES: Tera = {
        let mut tera = Tera::default();
        tera.add_raw_template("history.sql", SQL_HISTORY_TEMPLATE)
            .expect("failed to parse history.sql");
        tera.autoescape_on(vec![".sql"]);
        tera
    };
    pub static ref DEFAULT_HISTORY_CONTEXT: tera::Context = {
        tera::Context::from_value(serde_json::json!({
            "domain_event_tables": [
                "transfer",
                "new_owner",
                "new_resolver",
                "new_ttl",
                "wrapped_transfer",
                "name_wrapped",
                "name_unwrapped",
                "fuses_set",
                "expiry_extended",
            ],
            "resolver_event_tables": [
                "addr_changed",
                "multicoin_addr_changed",
                "name_changed",
                "abi_changed",
                "pubkey_changed",
                "text_changed",
                "contenthash_changed",
                "interface_changed",
                "authorisation_changed",
                "version_changed",
            ],
            "registration_event_tables": [
                "name_registered",
                "name_renewed",
                "name_transferred"
            ]
        }))
        .expect("failed to load history context")
    };
}

fn sql_events_of_domain(
    schema: &str,
    sort: EventSort,
    order: Order,
) -> Result<String, tera::Error> {
    let mut context = DEFAULT_HISTORY_CONTEXT.clone();
    context.insert("schema", schema);
    context.insert("sort", &sort.to_string());
    context.insert("order", &order.to_string());
    TEMPLATES.render("history.sql", &context)
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn events_sql_works() {
        let sql = sql_events_of_domain("sgd1", EventSort::BlockNumber, Order::Asc)
            .expect("failed to render history.sql");
        let expected = include_str!("history_expected.sql");
        assert_eq!(sql, expected);
    }
}
