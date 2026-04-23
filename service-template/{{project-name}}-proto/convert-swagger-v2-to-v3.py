import os
import json
from typing import Any
import sys

try:
    import yaml
except ImportError as e:
    print("yaml is not installed, please install it with `pip install pyyaml`")
    raise e
import requests
from pathlib import Path
import argparse
import difflib

# Default values
DEFAULT_SWAGGER_V2_SUFFIX = ".swagger.yaml"
DEFAULT_SWAGGER_V3_SUFFIX = ".swagger.openapi-v3.yaml"
DEFAULT_SWAGGER_DIR = "swagger/v1"
DEFAULT_SERVICE_NAME = "{{project-name}}"


def read_swagger(path: Path) -> Any | None:
    if not path.exists():
        print(f"❌ Error: Swagger file '{path}' was not found.")
        return None

    with open(path) as f:
        yaml_data = yaml.safe_load(f)
    return yaml_data


def print_strings_diff(a: str, b: str):
    d = difflib.Differ()
    print("a:", a)
    print("b:", b)
    lines = list(
        d.compare(
            a.splitlines(keepends=True),
            b.splitlines(keepends=True),
        )
    )
    for l in lines:
        print(l, end="")


def convert_swagger(
    swagger_dir: str,
    service_name: str,
    swagger_v2_suffix: str,
    swagger_v3_suffix: str,
    validate_only: bool,
) -> None:
    swagger_v2_path = Path(swagger_dir) / f"{service_name}{swagger_v2_suffix}"
    yaml_current_v3 = read_swagger(swagger_v2_path)
    if yaml_current_v3 is None:
        return

    print(f"📨 Sending file '{swagger_v2_path}' to a converter")
    response = requests.post(
        "https://converter.swagger.io/api/convert",
        headers={"accept": "application/json", "Content-Type": "application/json"},
        json=yaml_current_v3,
    )

    if response.status_code != 200:
        print(
            f"❌ Error: Failed to convert Swagger. Status code: {response.status_code}"
        )
        return

    print(f"🔄 Got a response from the converter: {response.status_code}")

    # Use text to preserve key order in output
    json_output_str = response.text
    yaml_output = json.loads(json_output_str)
    yaml_output_str = yaml.dump(yaml_output)

    swagger_v3_path = Path(swagger_dir) / f"{service_name}{swagger_v3_suffix}"
    if validate_only:
        yaml_current_v3 = read_swagger(swagger_v3_path)
        if yaml_current_v3 is None:
            return
        yaml_current_v3_str = yaml.dump(yaml_current_v3)

        if yaml_output_str == yaml_current_v3_str:
            print(f"✅ Swagger v3 is relevant and does not need an update")
        else:
            print(
                f"❌ Swaggers do not match. Difference between newly-generated and existing:"
            )
            print_strings_diff(yaml_output_str, yaml_current_v3_str)
            print(f"\n❌ Swagger v3 needs to be re-generated")
            sys.exit(1)

    else:
        print(f"💾 Writing converted file to '{swagger_v3_path}'")
        with open(swagger_v3_path, "w") as f:
            f.write(yaml_output_str)


def main():
    parser = argparse.ArgumentParser(
        description="Convert Swagger v2 to OpenAPI v3 using an online API."
    )
    parser.add_argument(
        "--swagger-dir",
        default=DEFAULT_SWAGGER_DIR,
        help="Directory containing the Swagger files (default: swagger/v1)",
    )
    parser.add_argument(
        "--service-name",
        default=DEFAULT_SERVICE_NAME,
        help="Service name used to identify the Swagger file (default: autoscout)",
    )
    parser.add_argument(
        "--v2-suffix",
        default=DEFAULT_SWAGGER_V2_SUFFIX,
        help="Suffix for the Swagger v2 file (default: .swagger.yaml)",
    )
    parser.add_argument(
        "--v3-suffix",
        default=DEFAULT_SWAGGER_V3_SUFFIX,
        help="Suffix for the converted Swagger v3 file (default: .swagger.openapi-v3.yaml)",
    )
    parser.add_argument(
        "--validate-only",
        action="store_true",
        help="Only check that the existing v3 config corresponds to v2",
    )

    args = parser.parse_args()

    convert_swagger(
        args.swagger_dir,
        args.service_name,
        args.v2_suffix,
        args.v3_suffix,
        args.validate_only,
    )


if __name__ == "__main__":
    main()
