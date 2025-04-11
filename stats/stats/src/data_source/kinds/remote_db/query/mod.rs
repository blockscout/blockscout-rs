mod all;
mod each;
mod one;

pub use all::{
    prepare_range_query_statement, PullAllWithAndSort, PullAllWithAndSortCached, StatementFromRange,
};
use chrono::{Days, NaiveDate};
pub use each::{PullEachWith, StatementFromTimespan};
pub use one::{
    PullOne, PullOne24hCached, PullOneNowValue, StatementForOne, StatementFromUpdateTime,
};
use sea_orm::{FromQueryResult, Statement};

use crate::{
    charts::db_interaction::read::{cached::find_one_value_cached, find_one_value},
    data_source::{types::Cacheable, UpdateContext},
    types::{timespans::DateValue, Timespan, TimespanDuration, TimespanTrait, TimespanValue},
    utils::day_start,
    ChartError,
};

// `DailyDataStatement` is assumed to have [`MissingDatePolicy::FillZero`]
pub(crate) async fn query_yesterday_data<DailyDataStatement: StatementFromRange>(
    cx: &UpdateContext<'_>,
    today: NaiveDate,
) -> Result<TimespanValue<NaiveDate, String>, ChartError> {
    let yesterday = calculate_yesterday(today)?;
    let query_statement = yesterday_statement::<DailyDataStatement>(cx, yesterday)?;
    let mut data = find_one_value::<DateValue<String>>(cx, query_statement)
        .await?
        // no data for yesterday
        .unwrap_or(TimespanValue::with_zero_value(yesterday));
    // today's value is the number from the day before.
    // still a value is considered to be "for today" (technically)
    data.timespan = today;
    Ok(data)
}

// `DailyDataStatement` is assumed to have [`MissingDatePolicy::FillZero`]
pub(crate) async fn query_yesterday_data_cached<DailyDataStatement, Value>(
    cx: &UpdateContext<'_>,
    today: NaiveDate,
) -> Result<Option<Value>, ChartError>
where
    DailyDataStatement: StatementFromRange,
    Value: FromQueryResult + TimespanTrait<Timespan = NaiveDate> + Cacheable + Clone,
{
    let yesterday = calculate_yesterday(today)?;
    let query_statement = yesterday_statement::<DailyDataStatement>(cx, yesterday)?;
    let data = find_one_value_cached::<Value>(cx, query_statement)
        .await?
        .map(|mut data| {
            // today's value is the number from the day before.
            // still a value is considered to be "for today" (technically)
            *data.timespan_mut() = today;
            data
        });
    Ok(data)
}

pub fn calculate_yesterday(today: NaiveDate) -> Result<NaiveDate, ChartError> {
    today
        .checked_sub_days(Days::new(1))
        .ok_or(ChartError::Internal(
            "Update time is incorrect: ~ minimum possible date".into(),
        ))
}

fn yesterday_statement<DailyDataStatement: StatementFromRange>(
    cx: &UpdateContext,
    yesterday: NaiveDate,
) -> Result<Statement, ChartError> {
    let today = yesterday.saturating_add(TimespanDuration::from_days(1));
    let yesterday_range = day_start(&yesterday)..day_start(&today);
    Ok(DailyDataStatement::get_statement(
        Some(yesterday_range),
        &cx.blockscout_applied_migrations,
        &cx.enabled_update_charts_recursive,
    ))
}
