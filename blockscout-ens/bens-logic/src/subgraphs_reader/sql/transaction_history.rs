use crate::{
    entity::subgraph::domain_event::DomainEventTransaction, subgraphs_reader::SubgraphReadError,
};
use lazy_static::lazy_static;
use sqlx::postgres::PgPool;
use tera::{Context, Tera};

pub async fn find_transaction_events(
    pool: &PgPool,
    schema: &str,
    id: &str,
) -> Result<Vec<DomainEventTransaction>, SubgraphReadError> {
    let sql =
        sql_events_of_domain(schema).map_err(|e| SubgraphReadError::Internal(e.to_string()))?;
    let transactions: Vec<DomainEventTransaction> =
        sqlx::query_as(&sql).bind(id).fetch_all(pool).await?;
    Ok(transactions)
}

const SQL_EVENTS_TEMPLATE: &str = include_str!("history.sql");

lazy_static! {
    pub static ref TEMPLATES: Tera = {
        let mut tera = Tera::default();
        tera.add_raw_template("history.sql", SQL_EVENTS_TEMPLATE)
            .expect("failed to parse history.sql");
        tera.autoescape_on(vec![".sql"]);
        tera
    };
    pub static ref DEFAULT_HISTORY_CONTEXT: Context = {
        Context::from_value(serde_json::json!({
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

fn sql_events_of_domain(schema: &str) -> Result<String, tera::Error> {
    let mut context = DEFAULT_HISTORY_CONTEXT.clone();
    context.insert("schema", schema);
    TEMPLATES.render("history.sql", &context)
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn events_sql_works() {
        let sql = sql_events_of_domain("sgd1").expect("failed to render history.sql");
        let expected = include_str!("history_expected.sql");
        assert_eq!(sql, expected);
    }
}
