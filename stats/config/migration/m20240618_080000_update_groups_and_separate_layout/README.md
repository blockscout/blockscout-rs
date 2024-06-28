# Config migration

As a result of change in internal logic, configuration is changed.
This "migration" is aimed to help with updating the config correspondingly.

## Changes

- `charts.json` now does not contain update scheduling information and chart layout. 
- Scheduling is configured per update group in a separate (`update_schedule.json`) file.
This file reflects update groups constructed within rust code.
- Layout is configured in `layout.json` file.

## How to migrate

### Updating config files
There is a script `split.py` that is aimed to greatly simplify the migration process

#### Run the script
```
mkdir new_configs
python3 migration/m20240618_080000_update_groups_and_separate_layout/split.py charts.json -o ./new_configs 
```
Toml files should work as well; just make sure `toml` library for python is installed.

The script is likely to prompt you to choose update schedules for some update groups. Since multiple charts with different
update schedules are combined into one group, it is not straightforward which schedule to choose. In general case, it makes sense to just combine the schedules like the following (but it's up to you to decide)
```
0 0 3 * * * * (chart 1)
0 0 7 * * * * (chart 2)
0 0 12 * * * * (chart 3)

> "0 0 3,7,12 * * * *"
```

#### Review the resulting configurations
It's better to skim through the files by hand. The script is not battle-tested, but it worked well on default config.

#### Replace the configs
Something like this:
```
mv ./new_configs/* ./
rm -r ./new_configs
```

## Existing env updating

There are respective changes to env variables.

### `STATS___`
Settings for the server are mostly unchanged. The only thing is, if you have non-default config location (`STATS___CHARTS_CONFIG`), you likely need to set corresponding `STATS___LAYOUT_CONFIG` and `STATS___UPDATE_GROUPS_CONFIG`.

### `STATS_CHARTS`
Unchanged variables:

- `STATS_CHARTS__COUNTERS__<NAME>__*` (except `..__UPDATE_SCHEDULE`) 
- `STATS_CHARTS__TEMPLATE_VALUES__*`

Below are instructions on migrating other env variables.

#### Update schedule

Counters:

- Old name - `STATS_CHARTS__COUNTERS__<NAME>__UPDATE_SCHEDULE`
- New name - `STATS_UPDATE_GROUPS__SCHEDULES__<GROUP_NAME>`
- Additional info - Schedule will be overridden for the whole group. `<GROUP_NAME>` is the name of update group
that contains the `<NAME>` chart. It can be found in [update_groups.rs](../../../stats-server/src/update_groups.rs) or
in [split.py (variable `update_groups_mapping`)](./split.py)

Line charts:

- Old name - `STATS_CHARTS__LINES__<1>__CHARTS__<2>__UPDATE_SCHEDULE`
- New name - `STATS_UPDATE_GROUPS__SCHEDULES__<GROUP_NAME>`
- Additional info - see Counters

#### Chart settings

Counters' settings are not touched.

- Old name - `STATS_CHARTS__LINES__<1>__CHARTS__<2>__*` (except `..__UPDATE_SCHEDULE`)
- New name - `STATS_CHARTS__LINE_CHARTS__<2>__*`

#### Line chart layout

Category settings:

- Old - `STATS_CHARTS__LINES__<1>__TITLE` or `STATS_CHARTS__LINES__<1>__ORDER`
- New - `STATS_LAYOUT__LINE_CHART_CATEGORIES__<1>__TITLE` or `STATS_LAYOUT__LINE_CHART_CATEGORIES__<1>__ORDER`

Chart location (order) in category:

- Old - `STATS_CHARTS__LINES__<1>__CHARTS__<2>` (i.e. chart `2` is in category `1`)
- New - `STATS_LAYOUT__LINE_CHART_CATEGORIES__<1>__CHARTS_ORDER__<2>=N`
- Additional info - `N` is the new place within the category
