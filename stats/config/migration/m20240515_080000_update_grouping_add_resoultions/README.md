# Config migration

As a result of change in internal logic, configuration is changed.
This "migration" is aimed to help with updating the config correspondingly.

## Changes
- `charts.json` now does not contain update scheduling information. It is moved
to a separate (`update_schedule.json`) file.
- This file reflects update groups constructed within rust code.
- Some members of a group can be disabled via corresponding `ignore_charts` property.

## How to apply
TODO

## Existing env updating
Only environmental variables starting with `STATS_CHARTS` (ones that affected the charts config) are affected.

Unchanged variables:
- `STATS_CHARTS__COUNTERS__<NAME>__*` (except `..__UPDATE_SCHEDULE`) 
- `STATS_CHARTS__TEMPLATE_VALUES__*`

Here are some examples on migrating the envs:
| Old name |  New name | Note |
|---|---|---|
| `STATS_CHARTS__COUNTERS__<NAME>__UPDATE_SCHEDULE` | `STATS_CHARTS__UPDATE_GROUPS__<GROUP_NAME>__UPDATE_SCHEDULE` | Schedule will be overridden for the whole group |
| `STATS_CHARTS__LINES__<1>__CHARTS__<2>__UPDATE_SCHEDULE` | `STATS_CHARTS__UPDATE_GROUPS__<GROUP_NAME>__UPDATE_SCHEDULE` | Schedule will be overridden for the whole group |
| `STATS_CHARTS__LINES__*` | `STATS_CHARTS__LINE_CATEGORIES__*` | Only `LINES` parts is changed |
