import json
import sys
import os


if __name__ == '__main__':
    args = sys.argv[1:]
    if len(args) != 1:
        print("you should provide only one argument - the root of directory to retrieve files from")
        exit(1)

    result = dict()

    root_dir = args[0]
    for subdir, dirs, files in os.walk(root_dir):
        if any(dir_to_ignore in subdir.split(os.sep) for dir_to_ignore in ['.git', 'target']):
            continue

        for file in files:
            if any(file.endswith(ext) for ext in ['.toml', 'lock', '.rs']):
                content = open(os.path.join(subdir, file), 'r').read()
                file_path = os.path.join(subdir.removeprefix(root_dir), file)
                result[file_path] = content

    print(json.dumps(result))

