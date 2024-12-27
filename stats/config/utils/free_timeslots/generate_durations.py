import pandas as pd
import re
import json
from typing import Dict, List, Set, Optional
import typer
from pathlib import Path


def convert_camel_to_snake(name: str) -> str:
    """Convert camelCase to snake_case."""
    name = re.sub("([a-z0-9])([A-Z])", r"\1_\2", name)
    return name.lower()


def normalize_period(period: str) -> str:
    """Normalize period names (WEEK -> Weekly, etc)."""
    period_map = {"WEEK": "Weekly", "MONTH": "Monthly", "YEAR": "Yearly", "DAY": ""}
    return period_map.get(period, period)


def parse_rust_groups(rust_file_path: str) -> Dict[str, List[str]]:
    """Parse Rust file to extract update groups and their charts."""
    with open(rust_file_path, "r") as f:
        content = f.read()

    # Extract singleton groups
    singleton_pattern = r"singleton_groups!\(([\s\S]*?)\);"
    singleton_match = re.search(singleton_pattern, content)

    groups = {}
    if singleton_match:
        # Extract individual chart names, skipping comments
        charts = re.findall(
            r"^\s*([A-Za-z0-9]+),?\s*(?://.*)?$", singleton_match.group(1), re.MULTILINE
        )

        # Create group names and entries for singleton groups
        for chart in charts:
            group_name = f"{chart}Group"
            groups[group_name] = [chart]

    # Extract complex groups
    group_pattern = (
        r"construct_update_group!\((\w+)\s*\{[\s\S]*?charts:\s*\[([\s\S]*?)\]"
    )
    complex_groups = re.finditer(group_pattern, content)

    for match in complex_groups:
        group_name = match.group(1)
        # Extract chart names, handling possible comments
        charts = re.findall(r"([A-Za-z0-9]+),", match.group(2))
        if charts:
            groups[group_name] = charts

    return groups


def process_durations(
    csv_path: Path, rust_path: Path, output_path: Path, verbose: bool = False
) -> Dict[str, int]:
    """Process duration data and create config file."""
    if verbose:
        print(f"Reading duration data from {csv_path}")

    # Read first row of CSV
    df = pd.read_csv(csv_path, nrows=1)

    # Get duration columns (skip 'Time' column)
    duration_cols = [col for col in df.columns if col != "Time"]

    if verbose:
        print(f"Found {len(duration_cols)} duration columns")

    # Create mapping of chart names to durations
    chart_durations = {}
    for col in duration_cols:
        # Split column name into chart and period
        parts = col.split("_")
        if len(parts) == 2:
            chart_name, period = parts

            # Convert to camelCase and normalize period if present
            camel_chart = "".join(
                word.capitalize()
                for word in convert_camel_to_snake(chart_name).split("_")
            )
            if period in ["WEEK", "MONTH", "YEAR", "DAY"]:
                camel_chart += normalize_period(period)

            # Store duration (convert to milliseconds and round to nearest minute)
            duration_mins = round(
                float(df[col].iloc[0]) / 60
            )  # assuming duration is in seconds
            chart_durations[camel_chart] = duration_mins

            if verbose:
                print(f"Processed chart {camel_chart}: {duration_mins} minutes")

    if verbose:
        print(f"\nParsing group definitions from {rust_path}")

    # Parse group definitions
    groups = parse_rust_groups(rust_path)

    if verbose:
        print(f"Found {len(groups)} update groups")

    # Calculate group durations
    group_durations = {}
    for group_name, charts in groups.items():
        total_duration = 0
        missing_charts = []
        matched_charts = []

        for chart in charts:
            if chart in chart_durations:
                total_duration += chart_durations[chart]
                matched_charts.append(chart)
            else:
                missing_charts.append(chart)

        # Convert group name to snake_case for consistency with visualizer
        snake_group = convert_camel_to_snake(group_name)
        group_durations[snake_group] = max(
            1, total_duration
        )  # ensure at least 1 minute

        if verbose:
            print(f"\nGroup: {snake_group}")
            print(f"Total duration: {group_durations[snake_group]} minutes")
            if missing_charts:
                print(
                    f"Warning: Missing duration data for charts: {', '.join(missing_charts)}"
                )
                if matched_charts:
                    print(
                        f"Duration data found for charts: {', '.join(matched_charts)}"
                    )
                else:
                    print(f"No charts in the group had duration data")

    # Save to JSON file
    output_path.parent.mkdir(parents=True, exist_ok=True)
    with open(output_path, "w") as f:
        json.dump(group_durations, f, indent=2)

    if verbose:
        print(f"\nSaved durations configuration to {output_path}")

    return group_durations


def main(
    csv_path: Path = typer.Argument(
        ...,
        help="Path to CSV file with duration data",
        exists=True,
        dir_okay=False,
        readable=True,
    ),
    rust_path: Path = typer.Option(
        Path("../../../stats/src/update_groups.rs"),
        help="Path to Rust file with group definitions",
        exists=True,
        dir_okay=False,
        readable=True,
    ),
    output_path: Path = typer.Option(
        Path("durations/durations.json"),
        "--output",
        "-o",
        help="Path for output JSON file",
        writable=True,
    ),
    verbose: bool = typer.Option(
        False, "--verbose", "-v", help="Enable verbose output"
    ),
    print_durations: bool = typer.Option(
        False, "--print", "-p", help="Print calculated durations"
    ),
):
    """
    Process update durations from CSV data and group definitions from Rust code.

    This tool reads duration data from a CSV file and group definitions from a Rust source file,
    calculates total durations for each update group, and saves the results to a JSON file
    that can be used by the visualization tool.
    """
    try:
        durations = process_durations(csv_path, rust_path, output_path, verbose)

        if print_durations:
            print("\nCalculated durations:")
            for group, duration in sorted(durations.items()):
                print(f"{group}: {duration} minutes")

    except Exception as e:
        typer.echo(f"Error: {str(e)}", err=True)
        raise typer.Exit(code=1)


if __name__ == "__main__":
    typer.run(main)
