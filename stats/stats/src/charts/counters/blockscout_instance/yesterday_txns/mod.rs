use crate::{
    chart_prelude::*, lines::NewTxnsCombinedStatement, types::new_txns::NewTxnsCombinedPoint,
};

pub mod all_yesterday_txns;
pub mod op_stack_yesterday_operational_txns;
pub use all_yesterday_txns::{YesterdayTxns, YesterdayTxnsInt};

pub struct YesterdayTxnsCombinedQuery;

impl RemoteQueryBehaviour for YesterdayTxnsCombinedQuery {
    type Output = NewTxnsCombinedPoint;

    async fn query_data(
        cx: &UpdateContext<'_>,
        _range: UniversalRange<DateTime<Utc>>,
    ) -> Result<Self::Output, ChartError> {
        let today = cx.time.date_naive();
        let yesterday = calculate_yesterday(today)?;
        let data = query_yesterday_data_cached::<NewTxnsCombinedStatement, NewTxnsCombinedPoint>(
            cx, today,
        )
        .await?
        // no data for yesterday
        .unwrap_or(NewTxnsCombinedPoint::with_zero_value(yesterday));
        Ok(data)
    }
}

pub type YesterdayTxnsCombinedRemote = RemoteDatabaseSource<YesterdayTxnsCombinedQuery>;
