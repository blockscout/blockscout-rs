//! Filter points that can be deduced according to `MissingDatePolicy`.
//! Can help with space usage efficiency.

use std::marker::PhantomData;

use blockscout_metrics_tools::AggregateTimer;
use chrono::{DateTime, Utc};
use rust_decimal::prelude::Zero;

use crate::{
    data_source::{kinds::AdapterDataSource, DataSource, UpdateContext},
    range::UniversalRange,
    types::TimespanValue,
    ChartError, ChartProperties, MissingDatePolicy,
};

/// Pass only essential points from `D`, removing ones that can be deduced
/// from MissingDatePolicy specified in `Properties`.
pub struct FilterDeducible<D, Properties>(PhantomData<(D, Properties)>)
where
    D: DataSource,
    Properties: ChartProperties;

impl<DS, Resolution, Value: Zero, Properties> AdapterDataSource for FilterDeducible<DS, Properties>
where
    DS: DataSource<Output = Vec<TimespanValue<Resolution, Value>>>,
    Resolution: Clone + Send,
    Value: PartialEq + Clone + Send,
    Properties: ChartProperties,
{
    type MainDependencies = DS;
    type ResolutionDependencies = ();
    type Output = DS::Output;

    async fn query_data(
        cx: &UpdateContext<'_>,
        range: UniversalRange<DateTime<Utc>>,
        dependency_data_fetch_timer: &mut AggregateTimer,
    ) -> Result<Self::Output, ChartError> {
        let data = DS::query_data(cx, range, dependency_data_fetch_timer).await?;
        Ok(match Properties::missing_date_policy() {
            MissingDatePolicy::FillZero => {
                data.into_iter().filter(|p| !p.value.is_zero()).collect()
            }
            MissingDatePolicy::FillPrevious => {
                let mut data = data.into_iter();
                let Some(mut previous) = data.next() else {
                    return Ok(vec![]);
                };
                let mut result = vec![previous.clone()];
                for next in data {
                    if next.value != previous.value {
                        result.push(next.clone());
                        previous = next;
                    }
                }
                result
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        data_source::{types::BlockscoutMigrations, UpdateParameters},
        gettable_const,
        lines::PredefinedMockSource,
        range::UniversalRange,
        tests::point_construction::{d_v_double, dt},
        types::timespans::DateValue,
        MissingDatePolicy, Named,
    };

    use super::*;

    use chrono::NaiveDate;
    use entity::sea_orm_active_enums::ChartType;
    use pretty_assertions::assert_eq;

    #[tokio::test]
    async fn filter_deducible_works() {
        gettable_const!(MockData: Vec<DateValue<f64>> = vec![
            d_v_double("2024-07-08", 5.0),
            d_v_double("2024-07-10", 5.0),
            d_v_double("2024-07-14", 10.3),
            d_v_double("2024-07-17", 5.0),
            d_v_double("2024-07-18", 10.3),
            d_v_double("2024-07-19", 0.0),
            d_v_double("2024-07-21", 0.0),
            d_v_double("2024-07-22", 10.3),
            d_v_double("2024-07-23", 5.0)
        ]);
        gettable_const!(PolicyZero: MissingDatePolicy = MissingDatePolicy::FillZero);
        gettable_const!(PolicyPrevious: MissingDatePolicy = MissingDatePolicy::FillPrevious);

        type PredefinedSourceZero = PredefinedMockSource<MockData, PolicyZero>;
        type PredefinedSourcePrevious = PredefinedMockSource<MockData, PolicyPrevious>;

        pub struct PropertiesZero;

        impl Named for PropertiesZero {
            fn name() -> String {
                "predefinedZero".into()
            }
        }
        impl ChartProperties for PropertiesZero {
            type Resolution = NaiveDate;

            fn chart_type() -> ChartType {
                ChartType::Line
            }
        }

        pub struct PropertiesPrevious;

        impl Named for PropertiesPrevious {
            fn name() -> String {
                "propertiesPrevious".into()
            }
        }
        impl ChartProperties for PropertiesPrevious {
            type Resolution = NaiveDate;

            fn chart_type() -> ChartType {
                ChartType::Line
            }

            fn missing_date_policy() -> MissingDatePolicy {
                MissingDatePolicy::FillPrevious
            }
        }

        type TestedZero = FilterDeducible<PredefinedSourceZero, PropertiesZero>;
        type TestedPrevious = FilterDeducible<PredefinedSourcePrevious, PropertiesPrevious>;

        // db is not used in mock
        let empty_db = sea_orm::Database::connect("sqlite::memory:").await.unwrap();

        let context =
            UpdateContext::from_params_now_or_override(UpdateParameters::query_parameters(
                &empty_db,
                &empty_db,
                BlockscoutMigrations::latest(),
                Some(dt("2024-07-30T09:00:00").and_utc()),
            ));
        assert_eq!(
            <TestedZero as DataSource>::query_data(
                &context,
                UniversalRange::full(),
                &mut AggregateTimer::new()
            )
            .await
            .unwrap(),
            vec![
                d_v_double("2024-07-08", 5.0),
                d_v_double("2024-07-10", 5.0),
                d_v_double("2024-07-14", 10.3),
                d_v_double("2024-07-17", 5.0),
                d_v_double("2024-07-18", 10.3),
                d_v_double("2024-07-22", 10.3),
                d_v_double("2024-07-23", 5.0)
            ]
        );
        assert_eq!(
            <TestedPrevious as DataSource>::query_data(
                &context,
                UniversalRange::full(),
                &mut AggregateTimer::new()
            )
            .await
            .unwrap(),
            vec![
                d_v_double("2024-07-08", 5.0),
                d_v_double("2024-07-14", 10.3),
                d_v_double("2024-07-17", 5.0),
                d_v_double("2024-07-18", 10.3),
                d_v_double("2024-07-19", 0.0),
                d_v_double("2024-07-22", 10.3),
                d_v_double("2024-07-23", 5.0)
            ]
        );
    }
}
