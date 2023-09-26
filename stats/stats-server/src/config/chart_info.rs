use cron::Schedule;
use serde::{Deserialize, Serialize};
use serde_with::{serde_as, DisplayFromStr};

#[serde_as]
#[derive(Default, Debug, Clone, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ChartSettings {
    #[serde(default = "enabled_default")]
    pub enabled: bool,
    #[serde_as(as = "Option<DisplayFromStr>")]
    pub update_schedule: Option<Schedule>,
    pub units: Option<String>,
}

fn enabled_default() -> bool {
    true
}

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct CounterInfo {
    pub title: String,
    pub description: String,
    #[serde(flatten)]
    pub settings: ChartSettings,
}

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct LineChartInfo {
    pub title: String,
    pub description: String,
    #[serde(flatten)]
    pub settings: ChartSettings,
}
