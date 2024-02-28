use serde::Deserialize;
use std::fmt::Display;

pub trait Paginator<I> {
    fn add_to_query(&self, query: &mut sea_query::SelectStatement) -> Result<(), anyhow::Error>;

    fn paginate_result(&self, items: Vec<I>) -> Result<PaginatedList<I>, anyhow::Error>;
}

#[derive(Debug, Clone)]
pub struct PaginationInput<S> {
    pub sort: S,
    pub order: Order,
    pub page_size: u32,
    pub page_token: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PaginatedList<I> {
    pub items: Vec<I>,
    pub next_page_token: Option<String>,
}

impl<I> PaginatedList<I> {
    pub fn empty() -> Self {
        Self {
            items: vec![],
            next_page_token: None,
        }
    }
}

macro_rules! paginate_list {
    ($items:ident, $page_size:expr, $order_field:ident) => {{
        let page_size = $page_size as usize;
        let (items, next_page_token) = match $items.get(page_size) {
            Some(item) => (
                $items[0..page_size].to_vec(),
                Some(item.$order_field.clone().to_string()),
            ),
            None => ($items, None),
        };

        PaginatedList {
            items,
            next_page_token,
        }
    }};
}
pub(crate) use paginate_list;

#[derive(Debug, Clone, Copy, Deserialize, Default)]
pub enum Order {
    #[default]
    Asc,
    Desc,
}

impl Order {
    pub fn is_desc(&self) -> bool {
        matches!(self, Order::Desc)
    }

    pub fn to_database_field(&self) -> sea_query::Order {
        match self {
            Order::Asc => sea_query::Order::Asc,
            Order::Desc => sea_query::Order::Desc,
        }
    }
}

impl Display for Order {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Order::Asc => write!(f, "asc"),
            Order::Desc => write!(f, "desc"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
    struct A {
        foo: i32,
        bar: u32,
    }

    impl A {
        fn new(foo: i32, bar: u32) -> Self {
            Self { foo, bar }
        }
    }

    #[test]
    fn it_works() {
        for (items, page_size, expected) in [
            (
                vec![],
                100,
                PaginatedList {
                    items: vec![],
                    next_page_token: None,
                },
            ),
            (
                vec![A::new(1, 2), A::new(2, 3), A::new(3, 4)],
                2,
                PaginatedList {
                    items: vec![A::new(1, 2), A::new(2, 3)],
                    next_page_token: Some("3".to_string()),
                },
            ),
            (
                vec![A::new(1, 2), A::new(2, 3), A::new(3, 4)],
                3,
                PaginatedList {
                    items: vec![A::new(1, 2), A::new(2, 3), A::new(3, 4)],
                    next_page_token: None,
                },
            ),
        ] {
            let actual = paginate_list!(items, page_size, foo);
            assert_eq!(expected, actual);
        }
    }
}
