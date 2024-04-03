# dir2prompt

[![PyPI version](https://badge.fury.io/py/dir2prompt.svg)](https://badge.fury.io/py/dir2prompt)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

`dir2prompt` is a Python package that generates prompts for long-context language models (LLMs) from the contents of a directory. It creates a tree representation of the directory structure and includes the contents of each file in the prompt, allowing you to easily feed the information from a directory to an LLM.

## Installation

You can install `dir2prompt` using pip:

```sh
pip install dir2prompt
```

## Usage

To generate a prompt from a directory, use the `d2p` command followed by the desired options:

```sh
d2p --dir /path/to/directory --filters .py .txt --ignore-dir .git __pycache__ --ignore-file .DS_Store
```

This command will generate a prompt for the specified directory, including only files with the extensions `.py` and `.txt`, ignoring the `.git` and `__pycache__` directories, and the `.DS_Store` file.

The generated prompt will be saved as a `.txt` file in the current directory with the name `<directory_name>_prompt.txt`.

## Options

--`dir`: The directory to generate the prompt for (default: current directory).

--`filters`: File extensions to include in the prompt (default: all files).

--`outpath`: The output path for the prompt file (default: current directory).

--`outfile`: The name of the output file (default: `<directory_name>_prompt`).

--`ignore-dir`: Additional directories to ignore (e.g., `.git`, `__pycache__`).

--`ignore-file`: Additional file types to ignore (e.g., `.DS_Store`, `.log`).

--`config`: Path to a custom configuration file (default: `config.json` in the package directory).


## Configuration

`dir2prompt` uses a configuration file (`config.json`) to specify the default directories and files to ignore. You can provide a custom configuration file using the `--config` option.

The default `config.json` file includes commonly ignored directories and files. You can modify this file or create your own to suit your needs.

Example
Here's an example of using `dir2prompt` to generate a prompt for a Python project directory:

```sh
d2p --dir /path/to/my/project --filters .py --ignore-dir .git __pycache__ --ignore-file .DS_Store
```

This command will generate a prompt for the project directory, including only `.py` files and ignoring the `.git` and `__pycache__` directories, and the `.DS_Store` file.

The generated prompt will have the following structure:

```xml
<context>
<directory_tree>
project/
├── README.md
├── requirements.txt
└── src/
    ├── __init__.py
    └── main.py
</directory_tree>

<files>

<file>
<path>README.md</path>
<content>
# Project Title

This is an example Python project.
</content>
</file>

<file>
<path>requirements.txt</path>
<content>
numpy==1.21.0
pandas==1.3.0
</content>
</file>

<file>
<path>src/__init__.py</path>
<content>
EMPTY FILE
</content>
</file>

<file>
<path>src/main.py</path>
<content>
import numpy as np
import pandas as pd

def main():
    print("Hello, World!")

if __name__ == "__main__":
    main()
</content>
</file>

</files>
</context>
```

You can then feed this prompt to an LLM to provide it with context about your project.

## License 

`dir2prompt` is released under the MIT License.

## Contributing

Contributions are welcome! If you find any issues or have suggestions for improvements, please open an issue or submit a pull request on the GitHub repository.


