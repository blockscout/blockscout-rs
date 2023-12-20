import os
import logging
import typing
import json
from cookiecutter import utils

logging.basicConfig(level=logging.DEBUG)
logger = logging.getLogger("post_gen_project")

def create_abis():
    OUTPUT_DIR = "abis"
    def gen_path(name):
        return f"{OUTPUT_DIR}/{name}.json"
    tasks: typing.Dict[str, str] = {}
    none_exists = False
    if "{{ cookiecutter.registry}}".lower() == "true":
        registry_abi = '{{ cookiecutter.registry_abi }}'
        tasks[gen_path("{{ cookiecutter.registry_name }}")] = registry_abi
    else:
        none_exists = True

    if "{{ cookiecutter.resolver}}".lower() == "true":
        resolver_abi = '{{ cookiecutter.resolver_abi}}'
        tasks[gen_path("{{ cookiecutter.resolver_name }}")] = resolver_abi
    else:
        none_exists = True

    if "{{ cookiecutter.controller}}".lower() == "true":
        controller_abi = '{{ cookiecutter.controller_abi}}'
        tasks[gen_path("{{ cookiecutter.controller_name }}")] = controller_abi
    else:
        none_exists = True

    if "{{ cookiecutter.base}}".lower() == "true":
        base_abi = '{{ cookiecutter.base_abi}}'
        tasks[gen_path("{{ cookiecutter.base_name }}")] = base_abi
    else:
        none_exists = True
    
    if none_exists:
        path = 'src/None.ts'
        logger.debug("Remove %s", path)
        os.remove(path)

    logger.info("Creating abis for %s", list(tasks.keys()))
    if not os.path.exists(OUTPUT_DIR):
        os.mkdir(OUTPUT_DIR)
    
    for path, abi in tasks.items():
        parsed_abi = json.loads(abi)
        with open(path, 'w') as f:
            json.dump(parsed_abi, f, indent=2)

if __name__ == '__main__':
    create_abis()
