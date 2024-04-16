use sea_orm::{DatabaseConnection, DbErr};

use crate::Chart;

pub async fn set_last_updated_at<C>(
    // chart: &C,
    chart_id: i32,
    db: &DatabaseConnection,
) -> Result<(), DbErr>
where
    C: Chart + ?Sized,
{
    Ok(())
}
