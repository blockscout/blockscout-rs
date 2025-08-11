use sea_orm::{
    ActiveModelTrait, ActiveValue, ColumnTrait, Condition, ConnectionTrait, DbErr, EntityName,
    EntityTrait, IntoActiveModel, IntoSimpleExpr, Iterable, PrimaryKeyToColumn, Value,
    prelude::Expr,
    sea_query::{
        Alias, ColumnRef, CommonTableExpression, Func, IntoIden, Query, UpdateStatement, ValueTuple,
    },
};
use thiserror::Error;

pub async fn batch_update<C, A>(db: &C, models: impl IntoIterator<Item = A>) -> Result<(), DbErr>
where
    C: ConnectionTrait,
    A: ActiveModelTrait,
{
    let models = models.into_iter().collect::<Vec<_>>();
    let models_count = models.len();
    let query = match prepare_batch_update_query(models.into_iter()) {
        Ok(query) => query,
        Err(PrepareBatchUpdateError::NoColumnsToUpdate) => {
            return Ok(());
        }
        Err(e) => return Err(DbErr::Custom(e.to_string())),
    };

    let stmt = db.get_database_backend().build(&query);
    let res = db.execute(stmt).await?;

    let rows_affected = res.rows_affected();
    if rows_affected != models_count as u64 {
        tracing::warn!(
            rows_affected = rows_affected,
            models_count = models_count,
            "number of rows updated does not match number of models to update",
        );
    }

    Ok(())
}

#[derive(Error, Debug)]
pub enum PrepareBatchUpdateError<A> {
    #[error("primary key is not set: {0:?}")]
    PrimaryKeyNotSet(A),
    #[error("no columns to update")]
    NoColumnsToUpdate,
}

// This is a modified version of the sea-orm batch insert query builder
// but adjusted for partial update queries.
// Note: Partial updates are handled by coalescing the original value with the new value.
// This implies that explicitly setting an ActiveValue to None will not overwrite the original value.
// https://github.com/SeaQL/sea-orm/blob/c87c0145f56e171b89a3967f95d8b6b7b743bd89/src/query/insert.rs#L173-L238
fn prepare_batch_update_query<A>(
    models: impl IntoIterator<Item = A>,
) -> Result<UpdateStatement, PrepareBatchUpdateError<A>>
where
    A: ActiveModelTrait,
{
    let cte_name = Alias::new("updates").into_iden();

    let mut columns_to_update: Vec<_> = <A::Entity as EntityTrait>::Column::iter()
        .map(|_| None)
        .collect();
    let mut null_value: Vec<Option<Value>> =
        std::iter::repeat_n(None, columns_to_update.len()).collect();
    let mut all_values: Vec<Vec<Option<Value>>> = Vec::new();

    for model in models.into_iter() {
        let mut am: A = model.into_active_model();

        // Each model must have a primary key value
        if am.get_primary_key_value().is_none() {
            return Err(PrepareBatchUpdateError::PrimaryKeyNotSet(am));
        }

        let mut values = Vec::with_capacity(columns_to_update.len());
        for (idx, col) in <A::Entity as EntityTrait>::Column::iter().enumerate() {
            let av = am.take(col);
            match av {
                ActiveValue::Set(value) | ActiveValue::Unchanged(value) => {
                    // Mark the column as used
                    columns_to_update[idx] = Some(col);
                    // Store the null value with the correct type
                    null_value[idx] = Some(value.as_null());
                    values.push(Some(value));
                }
                ActiveValue::NotSet => {
                    // Indicate a missing value
                    // When constructing the value tuple, this will be replaced
                    // with the actual typed null value
                    values.push(None);
                }
            }
        }
        all_values.push(values);
    }

    let value_columns = columns_to_update
        .iter()
        .cloned()
        .flatten()
        .collect::<Vec<_>>();
    // Filter out primary key columns
    let update_columns = value_columns
        .iter()
        .cloned()
        .filter(|c| <A::Entity as EntityTrait>::PrimaryKey::from_column(*c).is_none())
        .collect::<Vec<_>>();

    if update_columns.is_empty() {
        return Err(PrepareBatchUpdateError::NoColumnsToUpdate);
    }

    let value_tuples = all_values
        .into_iter()
        .map(|values| {
            let values = values
                .into_iter()
                .enumerate()
                .filter_map(|(i, v)| {
                    if columns_to_update[i].is_some() {
                        match v {
                            Some(value) => Some(value),
                            None => null_value[i].clone(),
                        }
                    } else {
                        None
                    }
                })
                .collect::<Vec<Value>>();
            ValueTuple::Many(values)
        })
        .collect::<Vec<_>>();

    // Map table columns to value columns
    let update_columns_mapping = update_columns.iter().map(|c| {
        (
            *c,
            Func::coalesce([
                c.save_as(Expr::col((cte_name.clone(), *c))),
                c.into_simple_expr(),
            ])
            .into(),
        )
    });

    // Match rows from CTE with rows from the table by primary key
    let mut conditions = Condition::all();
    for key in <A::Entity as EntityTrait>::PrimaryKey::iter() {
        let col = key.into_column();
        let cte_col = Expr::col((cte_name.clone(), col));
        let table_col = col.into_simple_expr();
        conditions = conditions.add(table_col.eq(cte_col));
    }
    let cte = CommonTableExpression::new()
        .query(
            Query::select()
                .column(ColumnRef::Asterisk)
                .from_values(value_tuples, cte_name.clone())
                .to_owned(),
        )
        .table_name(cte_name.clone())
        .columns(value_columns)
        .to_owned();

    let query = Query::update()
        .table(A::Entity::default().table_ref())
        .values(update_columns_mapping)
        .with_cte(cte)
        .from(cte_name)
        .cond_where(conditions)
        .to_owned();

    Ok(query)
}

#[cfg(test)]
mod tests {
    use super::*;
    use sea_orm::{
        ActiveValue::{NotSet, Set},
        entity::prelude::*,
        sea_query::PostgresQueryBuilder,
    };

    #[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq)]
    #[sea_orm(table_name = "test_model")]
    pub struct Model {
        #[sea_orm(primary_key)]
        pub id_1: i32,
        #[sea_orm(primary_key)]
        pub id_2: i32,
        pub f_1: i64,
        pub f_2: Vec<u8>,
        pub f_3: Option<String>,
        pub f_4: Option<String>,
    }

    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {}

    impl ActiveModelBehavior for ActiveModel {}

    #[test]
    fn test_batch_update() {
        let incomplete_pk_model = ActiveModel {
            id_1: Set(1),
            ..Default::default()
        };
        assert!(matches!(
            prepare_batch_update_query(vec![incomplete_pk_model]),
            Err(PrepareBatchUpdateError::PrimaryKeyNotSet(_))
        ));

        let empty_update = ActiveModel {
            id_1: Set(1),
            id_2: Set(2),
            ..Default::default()
        };
        assert!(matches!(
            prepare_batch_update_query(vec![empty_update]),
            Err(PrepareBatchUpdateError::NoColumnsToUpdate)
        ));

        // f_1 is set for the first model, not set for the second
        // f_2 is set for both models
        // f_3 is set for the second model, not set for the first
        // f_4 is not set for both models
        let models = vec![
            ActiveModel {
                id_1: Set(1),
                id_2: Set(2),
                f_1: Set(1),
                f_2: Set(vec![1, 2, 3]),
                f_3: NotSet,
                f_4: NotSet,
            },
            ActiveModel {
                id_1: Set(1),
                id_2: Set(2),
                f_1: NotSet,
                f_2: Set(vec![4, 5, 6]),
                f_3: Set(Some("test".to_string())),
                f_4: NotSet,
            },
        ];
        let query = prepare_batch_update_query(models).unwrap();
        assert_eq!(query.to_string(PostgresQueryBuilder), [
            r#"WITH "updates" ("id_1", "id_2", "f_1", "f_2", "f_3") AS"#,
            r#"(SELECT * FROM"#,
            r#"(VALUES (1, 2, 1, '\x010203', NULL), (1, 2, NULL, '\x040506', 'test')) AS "updates")"#,
            r#"UPDATE "test_model" SET"#,
            r#""f_1" = COALESCE("updates"."f_1", "test_model"."f_1"),"#,
            r#""f_2" = COALESCE("updates"."f_2", "test_model"."f_2"),"#,
            r#""f_3" = COALESCE("updates"."f_3", "test_model"."f_3")"#,
            r#"FROM "updates""#,
            r#"WHERE "test_model"."id_1" = "updates"."id_1" AND "test_model"."id_2" = "updates"."id_2""#,
        ].join(" "));
    }
}
