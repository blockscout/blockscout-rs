# Update timeslots visualization

## Preparations

1. Install tkinter (e.g. `apt-get install python3-tk` or `brew install python-tk`) for `find_free_timeslot.py` GUI
2. Install other dependencies from `requirements.txt`: `pip install -r requirements.txt`

## `find_free_timeslot.py`

It's a tool to roughly visualize the busyness of update schedule to find a timeslot for some new update group.

### Usage

Just run `python find_free_timeslot.py` and use GUI to find less crowded timeslots. 
You can regenerate durations config for more accurate representation.
See below for details

## Durations config

This is a script to generate a config for an accurate visualization within `find_free_timeslot` script.

### Usage

1. Get data fetch time statistics (e.g. from grafana) (example: `data.csv.example`).  In our case, you can:
    - Open "Microservices > Stats" dashboard
    - Find "Average data fetch time"
    - [Three dots] > [Inspect] > [Data]
    - [Data options] > [Show data frame] > [Series joined by time]
    - [Formatted data] = off
    - [Download CSV]
2. Run the script (preferably from this folder, to correctly use default parameters) (see `--help` for details)
3. Enjoy newly generated `durations.json` after running `find_free_timeslot` script.