use std::marker::PhantomData;

use chrono::{DateTime, Utc};
use sea_orm::{DatabaseConnection, DbErr};

use crate::{
    charts::db_interaction::write::create_chart,
    data_source::kinds::local_db::parameter_traits::CreateBehaviour, ChartProperties,
};

pub struct DefaultCreate<C: ChartProperties>(PhantomData<C>);

impl<C: ChartProperties> CreateBehaviour for DefaultCreate<C> {
    async fn create(db: &DatabaseConnection, init_time: &DateTime<Utc>) -> Result<(), DbErr> {
        create_chart(db, C::NAME.into(), C::chart_type(), init_time).await
    }
}
