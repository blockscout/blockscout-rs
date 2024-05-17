#[macro_export]
macro_rules! uuid_eq {
    ($col:expr, $val:expr) => {
        // NOTE: using entity_name() to get the table name
        // source: https://github.com/SeaQL/sea-orm/blob/010433bf1633dd387f8493071c0b3838a7fbb575/src/entity/column.rs#L30
        sea_orm::prelude::Expr::col(($col.entity_name(), $col))
            .cast_as(sea_orm::sea_query::Alias::new("text"))
            .eq($val)
    };
}
