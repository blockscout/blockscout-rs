// SPDX-License-Identifier: LicenseRef-Blockscout

use crate::subgraph::{DomainPaginationInput, Paginator};
use anyhow::Context;
use nonempty::NonEmpty;
use sea_query::{Alias, Asterisk, Expr, SelectStatement, UnionType};

pub fn bind_string_list(list: &[impl AsRef<str>]) -> Vec<String> {
    list.iter()
        .map(|s| s.as_ref().to_string())
        .collect::<Vec<_>>()
}

pub fn union_domain_queries(
    protocol_queries: NonEmpty<SelectStatement>,
    select_clause: Option<&str>,
    pagination: Option<&DomainPaginationInput>,
) -> Result<SelectStatement, anyhow::Error> {
    let select_clause = Expr::cust(select_clause.unwrap_or("*"));
    let sub_query = protocol_queries
        .into_iter()
        .enumerate()
        .map(|(i, mut q)| {
            if let Some(pagination) = pagination {
                pagination
                    .add_to_query(&mut q)
                    .context("adding pagination to query")?;
            }
            // sea_query renders the ORDER BY / LIMIT of the first statement of a
            // UNION after the whole union, so the first branch would lose its
            // own limit and scan the entire table of its protocol. Wrapping every
            // branch in a subquery keeps the limit inside each branch.
            Ok(SelectStatement::new()
                .column(Asterisk)
                .from_subquery(q, Alias::new(format!("branch_{i}")))
                .to_owned())
        })
        .collect::<Result<Vec<SelectStatement>, anyhow::Error>>()?
        .into_iter()
        .reduce(|mut acc, new| acc.union(UnionType::All, new).to_owned())
        .expect("reduce from non empty iterator");

    let mut query = SelectStatement::new();
    let q = query
        .expr(select_clause)
        .from_subquery(sub_query, Alias::new("sub"));

    if let Some(pagination) = pagination {
        pagination
            .add_to_query(q)
            .context("adding pagination to query")?;
    }

    Ok(q.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use sea_query::{PostgresQueryBuilder, Query};

    #[test]
    fn union_keeps_limit_in_every_branch() {
        let branch = |schema: &str| {
            Query::select()
                .column(Asterisk)
                .from((Alias::new(schema), Alias::new("domain")))
                .to_owned()
        };
        let pagination = DomainPaginationInput {
            sort: Default::default(),
            order: Default::default(),
            page_size: 50,
            page_token: None,
        };
        let queries = NonEmpty::from_vec(vec![branch("sgd1"), branch("sgd2"), branch("sgd3")])
            .expect("non empty");
        let sql = union_domain_queries(queries, None, Some(&pagination))
            .expect("failed to build union")
            .to_string(PostgresQueryBuilder);

        // a branch without its own limit scans the whole domain table of its
        // protocol, so the limit must come before the branch ends
        for schema in ["sgd1", "sgd2", "sgd3"] {
            let branch_sql = sql
                .split(&format!(r#""{schema}"."domain""#))
                .nth(1)
                .expect("branch is present");
            let branch_end = [branch_sql.find(')'), branch_sql.find("UNION")]
                .into_iter()
                .flatten()
                .min()
                .expect("branch is followed by union or closing parenthesis");
            assert!(
                branch_sql[..branch_end].contains("LIMIT 51"),
                "branch {schema} has no own limit: {sql}"
            );
        }
    }
}
