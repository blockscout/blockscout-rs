use super::{paginate_list, Order, PaginatedList, PaginationInput, Paginator};
use crate::{entity::subgraph::domain::Domain, subgraphs_reader::DomainSortField};
use anyhow::Context;
use sea_query::{Expr, SelectStatement, SimpleExpr};

pub type DomainPaginationInput = PaginationInput<DomainSortField>;

impl Paginator<Domain> for DomainPaginationInput {
    fn paginate_result(&self, items: Vec<Domain>) -> Result<PaginatedList<Domain>, anyhow::Error> {
        let list = match self.sort {
            DomainSortField::RegistrationDate => paginate_list!(items, self.page_size, created_at),
        };

        Ok(list)
    }

    fn add_to_query(&self, query: &mut SelectStatement) -> Result<(), anyhow::Error> {
        query
            .order_by(
                self.sort.to_database_field(),
                self.order.to_database_field(),
            )
            .limit(self.page_size as u64 + 1);

        if let Some(page_token) = self.page_token.as_ref() {
            let page_token = match self.sort {
                DomainSortField::RegistrationDate => SimpleExpr::from(
                    page_token
                        .parse::<u64>()
                        .context("cannot parse page_token for 'registration_date' sort")?,
                ),
            };
            let col = self.sort.to_database_field();
            let expr = match self.order {
                Order::Asc => Expr::col(col).gte(page_token),
                Order::Desc => Expr::col(col).lte(page_token),
            };
            query.and_where(expr);
        };

        Ok(())
    }
}
