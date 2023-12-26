use super::DomainSortField;
use crate::entity::subgraph::domain::Domain;

#[derive(Debug, Clone, PartialEq)]
pub struct PaginatedList<I, P> {
    pub items: Vec<I>,
    pub next_page_token: Option<P>,
}

macro_rules! paginate_list {
    ($items:ident, $page_size:expr, $order_field:ident) => {{
        let mut items = $items;
        let next_page_token = if items.len() > $page_size as usize {
            let next_page_token = items
                .drain($page_size as usize..)
                .last()
                .map(|item| item.$order_field);
            next_page_token
        } else {
            None
        };

        PaginatedList {
            items,
            next_page_token,
        }
    }};
}

pub fn paginate_domains(
    items: Vec<Domain>,
    sort: DomainSortField,
    page_size: u32,
) -> PaginatedList<Domain, String> {
    match sort {
        DomainSortField::RegistrationDate => {
            let paginated = paginate_list!(items, page_size, created_at);
            PaginatedList {
                items: paginated.items,
                next_page_token: paginated
                    .next_page_token
                    .map(|created_at| created_at.to_string()),
            }
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
                    next_page_token: Some(3),
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
