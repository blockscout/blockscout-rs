use crate::subgraphs_reader::{DomainPaginationInput, Paginator};
use anyhow::Context;
use nonempty::NonEmpty;
use sea_query::{Alias, Expr, SelectStatement, UnionType};

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
