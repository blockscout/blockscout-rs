#[macro_export]
macro_rules! uuid_eq {
    ($col:expr, $val:expr) => {
        sea_orm::prelude::Expr::col($col)
            .cast_as(sea_orm::sea_query::Alias::new("text"))
            .eq($val)
    };
}
