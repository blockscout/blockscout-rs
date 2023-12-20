import json
from collections import defaultdict


def abi_from_str(abi):
    abi = json.loads(abi)
    abi = sorted(abi, key=lambda x: x.get('name') or x.get('type'))
    return abi

def only_events(abi):
    return list(filter(lambda x: x.get('type') == 'event', abi))

def events_from_abi(abi):
    return list(map(lambda x: (x.get('name') + '(' + inputs_types(x['inputs']) + ')'), only_events(abi)))

def inputs_types(inputs):
    return ','.join(map(lambda x: x['type'], inputs))


def similar_event_exists(event, events_list: list):
    for maybe_event in events_list:
        if not maybe_event['name'].startswith(event['name']):
            continue

        i_types_count = defaultdict(int)
        for i in event['inputs']:
            i_types_count[i['type']] += 1
        j_types_count = defaultdict(int)
        for j in maybe_event['inputs']:
            j_types_count[j['type']] += 1
        for i in i_types_count:
            if i_types_count[i] > j_types_count[i]:
                break
        else:
            return maybe_event['name']
    
    return None