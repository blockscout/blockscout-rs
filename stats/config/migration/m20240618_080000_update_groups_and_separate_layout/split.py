import json
import itertools
import argparse
from collections import defaultdict
from copy import deepcopy


def camel_to_snake(input: str) -> str:
    result = ""
    for c in input:
        if c.isupper():
            result += "_" + c.lower()
        else:
            result += c
    return result

# perform conversions/transformations to make the config identical to json
def unify_toml(toml_dict: dict):
    for v in toml_dict["counters"]:
        v["id"] = camel_to_snake(v["id"])
    for v in toml_dict["lines"]:
        for chart in v["charts"]:
            chart["id"] = camel_to_snake(chart["id"])

# for testing
def render_json(json_dict: dict) -> dict:
    rendered_json = deepcopy(json_dict)
    def with_templates_replaced(s: any, template_values: dict):
        if type(s) is str:
            new_s = s
            for var_name in template_values:
                new_s = new_s.replace("{{" + var_name + "}}", template_values[var_name])
            return new_s
        else:
            return s

    template_values = rendered_json["template_values"]
    for counter in rendered_json["counters"]:
        for id in counter:
            counter[id] = with_templates_replaced(counter[id], template_values)
    for category in rendered_json["lines"]:
        category["title"] = with_templates_replaced(category["title"], template_values)
        for chart in category["charts"]:
            for id in chart:
                chart[id] = with_templates_replaced(chart[id], template_values)
    return rendered_json


def parse_json(json_data):
    result = {
        "template_values": json_data.get("template_values", {}),
        "counters": [],
        "lines": []
    }
    
    # Parse counters
    for key, value in json_data.get("counters", {}).items():
        value['id'] = key
        result["counters"].append(value)
    
    # Parse lines
    for section_key, section_value in json_data.get("lines", {}).items():
        section = {
            "id": section_key,
            "title": section_value.get("title"),
            "order": section_value.get("order"),
            "charts": []
        }
        for chart_key, chart_value in section_value.get("charts", {}).items():
            chart_value['id'] = chart_key
            section["charts"].append(chart_value)
        result["lines"].append(section)
    
    return result

def load_json(path):
    with open(path, 'r') as f:
        json_data = json.load(f)
    parsed_json = parse_json(json_data)
    return parsed_json


def parse_toml(toml_data):
    result = {
        "counters": [],
        "lines": []
    }
    
    # Parse counters
    for counter in toml_data.get("counters", []):
        result["counters"].append(counter)
    
    # Parse lines
    for section in toml_data.get("lines", {}).get("sections", []):
        section_dict = {
            "id": section.get("id"),
            "title": section.get("title"),
            "order": section.get("order"),
            "charts": []
        }
        for chart in section.get("charts", []):
            section_dict["charts"].append(chart)
        result["lines"].append(section_dict)
    
    return result

def load_toml(path):
    import toml
    with open(path, 'r') as f:
        toml_data = toml.load(f)
    parsed_toml = parse_toml(toml_data)
    unify_toml(parsed_toml)
    return parsed_toml


def load_file(path):
    try:
        return load_json(path)
    except Exception as json_e:
        try:
            return load_toml(path)
        except Exception as toml_e:
            print("Could not parse the file as json and toml:")
            print("json error:", json_e)
            print("toml error:", toml_e)
            raise Exception()

update_groups_mapping = {
    # singletons
    "active_accounts": "active_accounts_group",
    "average_block_rewards": "average_block_rewards_group",
    "average_block_size": "average_block_size_group",
    "average_gas_limit": "average_gas_limit_group",
    "average_gas_price": "average_gas_price_group",
    "average_txn_fee": "average_txn_fee_group",
    "gas_used_growth": "gas_used_growth_group",
    "native_coin_supply": "native_coin_supply_group",
    "new_blocks": "new_blocks_group",
    "txns_fee": "txns_fee_group",
    "txns_success_rate": "txns_success_rate_group",
    "average_block_time": "average_block_time_group",
    "completed_txns": "completed_txns_group",
    "total_addresses": "total_addresses_group",
    "total_blocks": "total_blocks_group",
    "total_tokens": "total_tokens_group",

    # NewAccountsGroup
    "new_accounts": "new_accounts_group",
    "accounts_growth": "new_accounts_group",
    "total_accounts": "new_accounts_group",

    # NewContractsGroup
    "new_contracts": "new_contracts_group",
    "contracts_growth": "new_contracts_group",
    "total_contracts": "new_contracts_group",
    "last_new_contracts": "new_contracts_group",
    
    # NewTxnsGroup
    "new_txns": "new_txns_group",
    "txns_growth": "new_txns_group",
    "total_txns": "new_txns_group",

    # NewVerifiedContractsGroup
    "new_verified_contracts": "new_verified_contracts_group",
    "verified_contracts_growth": "new_verified_contracts_group",
    "total_verified_contracts": "new_verified_contracts_group",
    "last_new_verified_contracts": "new_verified_contracts_group",
    
    # NativeCoinHoldersGrowthGroup
    "native_coin_holders_growth": "native_coin_holders_growth_group",
    "new_native_coin_holders": "native_coin_holders_growth_group",
    "total_native_coin_holders": "native_coin_holders_growth_group",

    # NewNativeCoinTransfersGroup
    "new_native_coin_transfers": "new_native_coin_transfers_group",
    "total_native_coin_transfers": "new_native_coin_transfers_group"
}

def line_charts_iter(parsed_config: dict) -> iter:
    return itertools.chain(*map(lambda l: l["charts"], parsed_config["lines"]))

def all_charts_iter(parsed_config: dict) -> iter:
    return itertools.chain(parsed_config["counters"], line_charts_iter(parsed_config))

def prompt_candidate_choice(group_name: str, candidates: list) -> int:
    print("{} - only one update schedule can be set for group (see `README.md` for details).".format(group_name))
    while True:
        for (i, c) in enumerate(candidates):
            print("{}: {}".format(i, c))
        choice = input("Type number ({}-{}) to choose or provide new cron expression in double quotes (e.g. \"<new expression>\"): ".format(0, len(candidates)-1)).strip()
        if choice[0] == '"' and choice[-1] == '"':
            return (choice[1:-1], 'custom')
        try:
            choice = int(choice)
            if choice >= 0 and choice < len(candidates):
                return candidates[choice]
        except Exception as e:
            print("error:", e)
            continue


def construct_update_groups(parsed_config: dict) -> dict:
    schedule_candidates = defaultdict(list)
    for chart_entry in all_charts_iter(parsed_config):
        id = chart_entry["id"]
        group_name = update_groups_mapping[id]
        schedule_candidates[group_name].append((chart_entry["update_schedule"], id))
    update_schedule = {}
    for (group_name, candidates) in schedule_candidates.items():
        chosen_schedule = candidates[0]
        if len(candidates) > 1:
            chosen_schedule = prompt_candidate_choice(group_name, candidates)
        update_schedule[group_name] = chosen_schedule[0]
    return { "schedules": update_schedule }

def construct_layout(parsed_config: dict) -> dict:
    layout = {}
    counters_order = []
    for counter in parsed_config["counters"]:
        counters_order.append(counter["id"])
    layout["counters_order"] = counters_order
    line_chart_categories = []
    for cat in parsed_config["lines"]:
        counters_order = list(map(lambda c: c["id"], cat["charts"]))
        layout_category = {
            "id": cat["id"],
            "title": cat["title"],
            "charts_order": counters_order,
        }
        line_chart_categories.append((cat["order"] or 9999, layout_category))
    line_chart_categories.sort(key=lambda c: c[0])
    layout["line_chart_categories"] = list(map(lambda p: p[1], line_chart_categories))
    return layout

def construct_charts(parsed_config: dict) -> dict:
    charts = {}
    if "template_values" in parsed_config:
        charts["template_values"] = parsed_config["template_values"]

    def chart_settings_without_id_and_update(c: dict) -> dict:
        new_c = c.copy()
        new_c.pop("id", None)
        new_c.pop("update_schedule", None)
        return new_c

    charts["counters"] = {c["id"]: chart_settings_without_id_and_update(c) for c in parsed_config["counters"]}
    charts["line_charts"] = {c["id"]: chart_settings_without_id_and_update(c) for c in line_charts_iter(parsed_config)}
    return charts

def save_config(path: str, config: dict):
    with open(path, 'x') as f:
        json.dump(config, f, indent=4)

if __name__ == "__main__":
    parser = argparse.ArgumentParser(prog="migrateConfigs",description="A script to simplify config migration to the new format")
    parser.add_argument('filename', help="old config file location")
    parser.add_argument('-o', '--output', metavar='output_folder', help="folder to put the results in")
    args = parser.parse_args()
    parsed_file = load_file(args.filename)

    if args.output is None:
        print("Please specify output folder")
        exit()

    save_config(args.output + "/charts.json", construct_charts(parsed_file))
    save_config(args.output + "/layout.json", construct_layout(parsed_file))
    save_config(args.output + "/update_groups.json", construct_update_groups(parsed_file))
