//! Synchronization for chart update.
//!
//! ## Reasoning
//!
//! Charts can be repeated across update groups

//todo: remove

use std::collections::{BTreeMap, HashSet};

use stats::UpdateError;
use tokio::sync::Mutex;

use crate::charts::ArcUpdateGroup;

struct GroupEntry {
    /// External ids (names) of charts in the group.
    /// Equivalent to chart names from
    /// [`UpdateGroup::list_charts`](stats::data_source::group::UpdateGroup::list_charts)
    chart_external_ids: HashSet<String>,
    handle: ArcUpdateGroup,
}

struct SyncGroups {
    groups: BTreeMap<String, ArcUpdateGroup>,
    chart_mutexes: BTreeMap<String, Mutex<()>>,
}

impl SyncGroups {
    pub fn update_group(&self, name: &str) -> Result<(), UpdateError> {
        let group_handle = self.groups.get(name).ok_or();
    }
}
