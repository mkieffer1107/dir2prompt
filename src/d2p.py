import os
import json
import fnmatch
import argparse
from importlib.resources import files


def build_directory_tree(
        dir: str = ".",
        path: str = "",
        level: int = 0,
        file_paths: list[str] = [],
        IGNORE_DIRS: list[str] = [],
        IGNORE_FILES: list[str] = [],
    ) -> tuple[str, list[str]]:
    """Build a tree representation of a directory and return a list of file paths under the root directory"""
    tree_str = ""

    if level == 0:
        # add the base directory name to the tree string
        tree_str += f"{os.path.basename(os.getcwd() if dir == '.' else dir)}/\n"

    # NOTE: this currently includes files to be ignored in tree string -- these should maybe be excluded as well
    # get all contents of the dir, ignoring dirs like build, target, etc. to save on token count for final tree string
    # using fnmatch to allow for wildcard patterns in IGNORE_DIRS
    items = [
        item
        for item in sorted(os.listdir(dir))
        if not any(fnmatch.fnmatch(item, pattern) for pattern in IGNORE_DIRS)
    ]

    for i, item in enumerate(items):
        item_path = os.path.join(dir, item)

        # the last item in the list (contents of curr dir) has not more items printed below it
        is_last_item = i == len(items) - 1
        if is_last_item:
            prefix = "└── " 
        else:
            prefix = "├── " 

        # when printing contents nested in child dirs, we need to make sure to print the
        # vertical bars to the left of these contents that connect the contents of the parent dir
        if level > 0:
            tree_str += "│   " * (level - 1)
            tree_str += "│   "

        # now add the item to the tree string and move to the next line for the next item
        if os.path.isdir(item_path):
            item += "/"
        tree_str += prefix + item + "\n"

        if os.path.isdir(item_path):
            # follow the directory down to the next level of the tree
            tree_str_child, file_paths = build_directory_tree(
                item_path,
                path=os.path.join(path, item),
                level=level + 1,
                file_paths=file_paths,
                IGNORE_DIRS=IGNORE_DIRS,
                IGNORE_FILES=IGNORE_FILES,
            )
            tree_str += tree_str_child
        else:
            # add file path to list if allowed file
            if not any(fnmatch.fnmatch(item, pattern) for pattern in IGNORE_FILES):
                file_paths.append(os.path.join(path, item))

    return tree_str, file_paths


def read_notebook(file: str) -> str:
    """Read the contents of a Jupyter notebook file (.ipynb) and return a string representation of the cells"""
    with open(file, "r") as f:
        notebook = json.load(f)
        cell_content = ["".join(cell["source"]) for cell in notebook["cells"]]
        cell_types = [cell["cell_type"] for cell in notebook["cells"]]
    
    notebook_str = ""
    for i, cell in enumerate(cell_content):
        notebook_str += f"{'-'*10} Cell {i+1} ({cell_types[i]}) {'-'*10}\n"
        notebook_str += cell + "\n\n"
    return notebook_str
    

def build_prompt(
        dir: str = ".", 
        filters: list[str] = None, 
        IGNORE_DIRS: list[str] = [], 
        IGNORE_FILES: list[str] = []
    ) -> str:
    """Build a prompt for a directory, including a tree representation of the directory and the contents of each file in the directory that matches the filters"""
    tree_str, file_paths = build_directory_tree(dir=dir, IGNORE_DIRS=IGNORE_DIRS, IGNORE_FILES=IGNORE_FILES)
    prompt = f"<context>\n"
    prompt += f"<directory_tree>\n{tree_str}</directory_tree>\n\n"

    prompt += "<files>\n\n"
    for file in file_paths:
        # read only filtered files, if specified
        if filters is None or any(file.endswith(ext) for ext in filters):
            try:
                if file.endswith(".ipynb"):
                    file_content = read_notebook(file)
                else:
                    with open(os.path.join(dir, file), "r") as f:
                        file_content = f.read()

                # add file string to prompt
                prompt += f"<file>\n"
                prompt += f"<path>{file}</path>\n"
                if not file_content.strip():
                    file_content = "EMPTY FILE"
                prompt += f"<content>\n{file_content}\n</content>\n"
                prompt += f"</file>\n\n"
                
            except UnicodeDecodeError:
                print(f"Unable to decode file content due to UnicodeDecodeError: {file}")
            except FileNotFoundError:
                print(f"File not found: {file}")

    prompt += "</files>\n"
    prompt += "</context>"
    return prompt


def load_config(config_path: str) -> dict:
    try:
        with open(config_path) as f:
            return json.load(f)
    except FileNotFoundError:
        raise Exception(f"Config file not found: {config_path}")
    

def save_file(contents: str, outpath: str = ".", outfile: str = "out"):
    path = os.path.join(outpath, f"{outfile}.txt")
    with open(path, "w") as f:
        f.write(contents)


def parse_options():
    parser = argparse.ArgumentParser(description="Generate a prompt for a directory")
    parser.add_argument("--dir", type=str, default=".", help="Directory to generate prompt for")
    parser.add_argument("--filters", type=str, nargs="+", help="File extensions to filter for")
    parser.add_argument("--outpath", type=str, default=".", help="Output path for prompt file")
    parser.add_argument("--outfile", type=str, help="Output file name for prompt file (default: <dir>_prompt)")
    parser.add_argument("--ignore-dir", type=str, nargs="+", help="Additional directories to ignore: specify directory names (e.g., .git, __pycache__, etc.)")
    parser.add_argument("--ignore-file", type=str, nargs="+", help="Additional file types to ignore: specify extensions with or without dot (e.g., py, ipynb, .c, etc.)")
    parser.add_argument("--config", type=str, help="Path to the custom configuration file (default: config.json)")
    args = parser.parse_args()

    # set the outfile name
    if args.outfile is None:
        if args.dir == ".":
            # replace "." with the actual base directory name
            dir_name = os.path.basename(os.getcwd()) 
        else:
            dir_name = os.path.basename(args.dir)
        args.outfile = f"{dir_name}_prompt"

    # set the default config file path 
    if args.config is None:
        args.config = str(files("src").joinpath("config.json"))
    return args


def main():
    args = parse_options()

    try:
        config = load_config(args.config)
    except Exception as e:
        raise Exception(f"Config file not found: {args.config}") from e

    IGNORE_DIRS = config["IGNORE_DIRS"]
    IGNORE_FILES = config["IGNORE_FILES"]

    # extend the default ignore lists with cli args
    if args.ignore_dir:
        IGNORE_DIRS.extend(args.ignore_dir)
    if args.ignore_file:
        IGNORE_FILES.extend(args.ignore_file)

    prompt = build_prompt(dir=args.dir, filters=args.filters, IGNORE_DIRS=IGNORE_DIRS, IGNORE_FILES=IGNORE_FILES)

    save_file(prompt, outpath=args.outpath, outfile=args.outfile)
    
    print(f"Prompt saved to {os.path.join(args.outpath, args.outfile)}.txt")


if __name__ == "__main__":
    main()