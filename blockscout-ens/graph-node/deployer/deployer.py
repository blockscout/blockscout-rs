import os.path
import time

import argparse
import json
import subprocess
import sys
import glob

DEFAULT_PROD_IFPS_URL = 'http://ipfs.node.blockscout.com'
DEFAULT_IPFS_URL = "http://127.0.0.1:5001"


def parse_args():
    parser = argparse.ArgumentParser(description="Script for parsing protocol name and config path.")
    parser.add_argument('protocol', type=str, help="Name of the protocol", default='')
    parser.add_argument('--config', type=str, default='config.json',
                        help="Path to the config file (default: config.json)")
    parser.add_argument('--prod', action='store_true', help="Deploy to production (default: false)")
    parser.add_argument('--version',
                        type=str, default='v0.0.1', help="Version of the subgraph to deploy (default: v0.0.1)")
    parser.add_argument('--graph-node-url', type=str, default="http://127.0.0.1:8020", )
    parser.add_argument('--ipfs-url', type=str, default=None)
    return parser.parse_args()


def load_config(config_path):
    with open(config_path, 'r') as file:
        config = json.load(file)
    return config


def exec_on_shell(cmd):
    pprint(f'executing command: {cmd}')
    process = subprocess.Popen(cmd, shell=True, stdout=subprocess.PIPE, stderr=subprocess.PIPE)

    while True:
        stdout_line = process.stdout.readline()
        stderr_line = process.stderr.readline()

        if stdout_line:
            sys.stdout.buffer.write(b"[EXEC::OUT] " + stdout_line)

        if stderr_line:
            sys.stderr.buffer.write(b"[EXEC::ERR] " + stderr_line)

        # Check if both stdout and stderr are empty and the process has terminated
        if process.poll() is not None and not stdout_line and not stderr_line:
            break
    process.wait()

    if process.returncode != 0:
        return error(f'Command failed with exit code {process.returncode}')


def colorize(text, color):
    """
    Colorize text using ANSI escape sequences.
    """
    colors = {
        'red': '\033[91m',
        'green': '\033[92m',
        'yellow': '\033[93m',
        'blue': '\033[94m',
        'magenta': '\033[95m',
        'cyan': '\033[96m',
        'white': '\033[97m',
        'reset': '\033[0m'
    }
    return f"{colors[color]}{text}{colors['reset']}"


def error(msg):
    pprint(colorize(f'Error: {msg}', 'red'))
    exit(1)


def pprint(msg):
    print('[DEPLOYER] ' + msg)


def deploy_subgraph(protocol, args):
    subgraph_path = protocol['subgraph_path']
    network = protocol['network']
    subgraph_name = protocol['subgraph_name']
    graph_node_url = args.graph_node_url
    version = args.version
    if args.ipfs_url is not None:
        ipfs_url = args.ipfs_url
    elif args.prod:
        ipfs_url = DEFAULT_PROD_IFPS_URL
    else:
        ipfs_url = DEFAULT_IPFS_URL

    package_path = subgraph_path + '/package.json'
    # check that the justfile exists
    if not os.path.exists(package_path):
        return error(f'package.json not found at {package_path}')

    exec_on_shell(f'yarn --cwd {subgraph_path} install')
    exec_on_shell(f'yarn --cwd {subgraph_path} codegen')

    original_contents = {}
    try:
        original_contents = process_files(subgraph_path, network)
        exec_on_shell(f'yarn --cwd {subgraph_path} build')
        exec_on_shell(f'yarn --cwd {subgraph_path} graph create --node {graph_node_url} {subgraph_name}')

        if args.prod:
            inpt = input('Are you sure you want to deploy to production? (y/n): ')
            if inpt.lower() != 'y':
                return error('Aborted deployment')

        exec_on_shell(
            f'yarn --cwd {subgraph_path} graph deploy --node {graph_node_url} --ipfs {ipfs_url} {subgraph_name} --network {network} --version-label {version}'
        )
    finally:
        return_original_files(original_contents)


def process_files(path, network):
    pprint("processing and templating files")
    file_pattern = 'src/*.ts'
    search_pattern = os.path.join(path, file_pattern)
    # Dictionary to store original file contents
    original_contents = {}

    # Find all matching files
    files = glob.glob(search_pattern)

    # Read each file, replace the placeholder, and save the modified content
    for file_path in files:
        with open(file_path, 'r', encoding='utf-8') as file:
            content = file.read()
            original_contents[file_path] = content
            modified_content = template_content(content, network)

        with open(file_path, 'w', encoding='utf-8') as file:
            file.write(modified_content)

    return original_contents


def return_original_files(original_contents):
    pprint(f"returning original contents: {original_contents.keys()}")
    # Restore the original file contents
    for file_path, content in original_contents.items():
        with open(file_path, 'w', encoding='utf-8') as file:
            file.write(content)


def template_content(content, network):
    content = content.replace("{{network}}", network)
    return content


def main():
    args = parse_args()
    config = load_config(args.config)
    protocols = config['protocols']
    protocol_name = args.protocol

    if protocol_name not in protocols:
        return error(
            f'Protocol "{protocol_name}" not found in config. possible options:\n' + "\n".join(protocols.keys()))
    protocol = protocols[protocol_name]
    deploy_subgraph(protocol, args)


if __name__ == "__main__":
    main()
