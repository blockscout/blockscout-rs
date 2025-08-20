use sea_orm::{
    EntityTrait, Order, QueryFilter, QueryOrder, QuerySelect, Select, Value,
    sea_query::{ExprTrait, IntoValueTuple, NullOrdering, SimpleExpr, ValueTuple},
};

#[derive(Debug, Clone)]
pub struct Cursor {
    pub page_token: Option<ValueTuple>,
    pub specs: Vec<KeySpec>,
}

impl Cursor {
    pub fn new(page_token: Option<impl IntoValueTuple>, specs: Vec<KeySpec>) -> Self {
        Self {
            page_token: page_token.map(|k| k.into_value_tuple()),
            specs,
        }
    }

    pub fn apply_pagination<E: EntityTrait>(
        &self,
        base: Select<E>,
        opts: PageOptions,
    ) -> Select<E> {
        let mut q = base;

        if let Some(expr) = self.build_where_expr() {
            q = q.filter(expr);
        };

        for k in self.specs.iter() {
            q = push_order_for_key_with_nulls(q, k.expr.clone(), k.dir.clone(), k.nulls);
        }

        q.limit(opts.limit)
    }

    pub fn build_where_expr(&self) -> Option<SimpleExpr> {
        let page_token = match self.page_token {
            Some(ref page_token) => page_token.clone(),
            None => return None,
        };

        let vals = page_token.clone().into_iter();

        let mut expr = SimpleExpr::Value(Value::Bool(Some(false)));

        for (k, v) in self.specs.iter().zip(vals).rev() {
            expr = fold_key(k, &v, expr);
        }

        Some(expr)
    }
}

pub struct PageOptions {
    pub limit: u64,
}

#[derive(Clone, Debug)]
pub struct KeySpec {
    pub expr: SimpleExpr,
    pub dir: Ordering,
    pub nulls: NullOrdering,
    pub nullable: bool,
}

impl KeySpec {
    pub fn new(expr: SimpleExpr, dir: Ordering, nulls: NullOrdering) -> Self {
        Self {
            expr,
            dir,
            nulls,
            nullable: false,
        }
    }

    pub fn nullable(mut self) -> Self {
        self.nullable = true;
        self
    }

    pub fn reversed(&self) -> Self {
        let dir = match self.dir {
            Ordering::Asc => Ordering::Desc,
            Ordering::Desc => Ordering::Asc,
        };
        let nulls = match self.nulls {
            NullOrdering::First => NullOrdering::Last,
            NullOrdering::Last => NullOrdering::First,
        };
        Self {
            dir,
            nulls,
            ..self.clone()
        }
    }
}

#[derive(Clone, Debug)]
pub enum Ordering {
    Asc,
    Desc,
}

impl From<Ordering> for Order {
    fn from(order: Ordering) -> Self {
        match order {
            Ordering::Asc => Order::Asc,
            Ordering::Desc => Order::Desc,
        }
    }
}

fn push_order_for_key_with_nulls<E: sea_orm::EntityTrait>(
    sel: sea_orm::Select<E>,
    expr: SimpleExpr,
    dir: Ordering,
    nulls: NullOrdering,
) -> sea_orm::Select<E> {
    let order = dir.into();
    sel.order_by_with_nulls(expr, order, nulls)
}

fn fold_key(k: &KeySpec, v: &Value, suffix: SimpleExpr) -> SimpleExpr {
    let cmp = cmp_expr(&k.expr, &k.dir, v);
    let eq_suffix = k
        .expr
        .clone()
        .eq(SimpleExpr::Value(v.clone()))
        .and(suffix.clone());
    let is_null = k.expr.clone().is_null();
    let is_not_null = k.expr.clone().is_not_null();
    let is_value_null = &v.as_null() == v;

    match (k.nullable, k.nulls, is_value_null) {
        // non-nullable: (col *cmp* v) OR (col = v AND suffix)
        (false, _, _) => cmp.or(eq_suffix),
        // nullable, NULLS LAST, cursor NULL: (col IS NULL AND suffix)
        (true, NullOrdering::Last, true) => is_null.and(suffix),
        // nullable, NULLS LAST, cursor NOT NULL: (col IS NULL) OR (col *cmp* v) OR (col = v AND suffix)
        (true, NullOrdering::Last, false) => is_null.or(cmp).or(eq_suffix),
        // nullable, NULLS FIRST, cursor NULL: (col IS NULL AND suffix) OR (col IS NOT NULL)
        (true, NullOrdering::First, true) => is_null.and(suffix).or(is_not_null),
        // nullable, NULLS FIRST, cursor NOT NULL: (col *cmp* v) OR (col = v AND suffix)
        (true, NullOrdering::First, false) => cmp.or(eq_suffix),
    }
}

fn cmp_expr(col: &SimpleExpr, dir: &Ordering, v: &Value) -> SimpleExpr {
    match dir {
        Ordering::Asc => col.clone().gt(SimpleExpr::Value(v.clone())),
        Ordering::Desc => col.clone().lt(SimpleExpr::Value(v.clone())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sea_orm::{
        IntoSimpleExpr, QueryTrait, entity::prelude::*, sea_query::PostgresQueryBuilder,
    };

    #[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq)]
    #[sea_orm(table_name = "test_model")]
    pub struct Model {
        #[sea_orm(primary_key)]
        pub id: i32,
        pub f_1: i64,
        pub f_2: Vec<u8>,
        pub f_3: Option<String>,
        pub f_4: Option<String>,
    }

    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {}

    impl ActiveModelBehavior for ActiveModel {}

    fn normalize_sql(statement: &str) -> String {
        statement.split_whitespace().collect::<Vec<_>>().join(" ")
    }

    #[test]
    fn test_cursor() {
        let specs = vec![
            KeySpec::new(
                Column::F1.into_simple_expr(),
                Ordering::Desc,
                NullOrdering::Last,
            ),
            KeySpec::new(
                Column::F2.into_simple_expr(),
                Ordering::Desc,
                NullOrdering::Last,
            ),
            KeySpec::new(
                Column::F3.into_simple_expr(),
                Ordering::Asc,
                NullOrdering::Last,
            )
            .nullable(),
            KeySpec::new(
                Column::F4.into_simple_expr(),
                Ordering::Asc,
                NullOrdering::Last,
            )
            .nullable(),
            KeySpec::new(
                Column::Id.into_simple_expr(),
                Ordering::Desc,
                NullOrdering::First,
            ),
        ];

        let cursor = Cursor::new(
            Some((123, vec![1, 2, 3], None::<String>, "test", 42)),
            specs,
        );

        let select = cursor.apply_pagination(Entity::find(), PageOptions { limit: 50 });

        let sql = select.as_query().to_string(PostgresQueryBuilder);
        let expected = r#"
            SELECT test_model.id,
              test_model.f_1,
              test_model.f_2,
              test_model.f_3,
              test_model.f_4
            FROM test_model
            WHERE test_model.f_1 < 123
              OR (
                test_model.f_1 = 123
                AND (
                  test_model.f_2 < ARRAY [1,2,3]
                  OR (
                    test_model.f_2 = ARRAY [1,2,3]
                    AND (
                      test_model.f_3 IS NULL
                      AND (
                        test_model.f_4 IS NULL
                        OR test_model.f_4 > 'test'
                        OR (
                          test_model.f_4 = 'test'
                          AND (
                            test_model.id < 42
                            OR (
                              test_model.id = 42
                              AND FALSE
                            )
                          )
                        )
                      )
                    )
                  )
                )
              )
            ORDER BY test_model.f_1 DESC NULLS LAST,
              test_model.f_2 DESC NULLS LAST,
              test_model.f_3 ASC NULLS LAST,
              test_model.f_4 ASC NULLS LAST,
              test_model.id DESC NULLS FIRST
            LIMIT 50
        "#;

        assert_eq!(normalize_sql(expected), normalize_sql(&sql));
    }
}
