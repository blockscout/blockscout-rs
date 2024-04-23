use entity::chart_data;
use sea_orm::{prelude::*, sea_query, ConnectionTrait};

pub async fn insert_data_many<C, D>(db: &C, data: D) -> Result<(), DbErr>
where
    C: ConnectionTrait,
    D: IntoIterator<Item = chart_data::ActiveModel> + Send + Sync,
{
    let mut data = data.into_iter().peekable();
    if data.peek().is_some() {
        chart_data::Entity::insert_many(data)
            .on_conflict(
                sea_query::OnConflict::columns([
                    chart_data::Column::ChartId,
                    chart_data::Column::Date,
                ])
                .update_column(chart_data::Column::Value)
                .to_owned(),
            )
            .exec(db)
            .await?;
    }
    Ok(())
}
