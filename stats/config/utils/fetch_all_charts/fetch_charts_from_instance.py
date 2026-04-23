#!/usr/bin/env python3
import argparse
import json
import urllib.request

DEFAULT_URL = "https://eth.blockscout.com/stats-service/api/v1/lines"


def fetch_chart_names(url: str) -> list[str]:
    with urllib.request.urlopen(url) as response:
        data = json.loads(response.read().decode())
    
    chart_names = []
    for section in data.get("sections", []):
        for chart in section.get("charts", []):
            if "id" in chart:
                chart_names.append(chart["id"])
    
    return chart_names


def main():
    parser = argparse.ArgumentParser(description="Fetch chart names from a blockscout stats instance")
    parser.add_argument("url", nargs="?", default=DEFAULT_URL, help=f"URL to fetch (default: {DEFAULT_URL})")
    args = parser.parse_args()

    chart_names = fetch_chart_names(args.url)
    output = {"chart_names": chart_names}
    print(json.dumps(output, indent=2))


if __name__ == "__main__":
    main()

