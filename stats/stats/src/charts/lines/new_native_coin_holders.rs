use super::NativeCoinHoldersGrowth;
use crate::{
    charts::{
        create_chart,
        insert::{DateValue, DateValueInt},
        updater::ChartDependentUpdater,
        Chart,
    },
    UpdateError,
};
use async_trait::async_trait;
use entity::sea_orm_active_enums::ChartType;
use sea_orm::prelude::*;
use std::sync::Arc;

pub struct NewNativeCoinHolders {
    parent: Arc<NativeCoinHoldersGrowth>,
}

impl NewNativeCoinHolders {
    pub fn new(parent: Arc<NativeCoinHoldersGrowth>) -> Self {
        Self { parent }
    }
}

#[async_trait]
impl ChartDependentUpdater<NativeCoinHoldersGrowth> for NewNativeCoinHolders {
    fn parent(&self) -> Arc<NativeCoinHoldersGrowth> {
        self.parent.clone()
    }

    async fn get_values(
        &self,
        mut parent_data: Vec<DateValue>,
    ) -> Result<Vec<DateValue>, UpdateError> {
        parent_data.sort();
        let data: Result<Vec<_>, _> = parent_data
            .into_iter()
            .map(DateValueInt::try_from)
            .scan(0, |prev, point| {
                Some(point.map(|mut point| {
                    let new = point.value;
                    point.value -= *prev;
                    *prev = new;
                    point
                }))
            })
            .map(|point| point.map(DateValue::from))
            .collect();
        Ok(data.map_err(|e| {
            let parent_name = self.parent.name();
            UpdateError::Internal(format!(
                "failed to parse values in chart '{parent_name}': {e}",
            ))
        })?)
    }
}

#[async_trait]
impl Chart for NewNativeCoinHolders {
    fn name(&self) -> &str {
        "newNativeCoinHolders"
    }

    fn chart_type(&self) -> ChartType {
        ChartType::Line
    }

    async fn create(&self, db: &DatabaseConnection) -> Result<(), DbErr> {
        self.parent.create(db).await?;
        create_chart(db, self.name().into(), self.chart_type()).await
    }

    async fn update(
        &self,
        db: &DatabaseConnection,
        blockscout: &DatabaseConnection,
        force_full: bool,
    ) -> Result<(), UpdateError> {
        self.update_with_values(db, blockscout, force_full).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::simple_test::simple_test_chart;

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_new_native_coin_holders() {
        let chart = NewNativeCoinHolders::new(Arc::new(NativeCoinHoldersGrowth::default()));

        simple_test_chart(
            "update_new_native_coin_holders",
            chart,
            vec![
                ("2022-11-09", "8"),
                ("2022-11-10", "0"),
                ("2022-11-11", "-1"),
            ],
        )
        .await;
    }
}
