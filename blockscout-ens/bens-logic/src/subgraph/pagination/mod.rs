mod domain;
mod paginator;

pub use domain::DomainPaginationInput;
pub(crate) use paginator::paginate_list;
pub use paginator::{Order, PaginatedList, PaginationInput, Paginator};
