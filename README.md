# dir2prompt

[![PyPI version](https://img.shields.io/pypi/v/dir2prompt.svg)](https://pypi.org/project/dir2prompt/)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

When you feel too lazy to selectively copy code from all the various files in your project, just copy it all! Inspired by [repo2prompt](https://github.com/andrewgcodes/repo2prompt).

## Installation 💻

You can install `dir2prompt` using pip:

```sh
pip install dir2prompt
```

## Usage 🚀

To generate a prompt from a directory, use the `d2p` command followed by the desired options:

```sh
d2p [directory path] --filters [file extensions] --outpath [output path] --outfile [output file name] --ignore-dir [directories to ignore] --ignore-file [files to ignore] --config [path to config file]

```

For ease of use, you can select a directory by passing it in as a positional argument

```sh
d2p [directory path]
```

## Options ⚙️

`--filter`: File extensions to include in the prompt (default: all files).

`--outpath`: The output path for the prompt file (default: current directory).

`--outfile`: The name of the output file (default: `<directory_name>_prompt`).

`--ignore-dir`: Additional directories to ignore (e.g., `experiments`, `run*`).

`--ignore-file`: Additional file types to ignore (e.g., `pt`, `rs`).

`--config`: Path to a custom config file (default: `config.json` in the package directory).

`--clean`: Remove all `<folder>_prompt.txt` files based on discovered directories.

## Example 🌟

Here's an example of how to use `dir2prompt` to generate a prompt:

```sh
d2p --filter py txt md ipynb --ignore-dir experiments __pycache__ --ignore-file old.py
```

This command will generate a prompt for the specified directory, including only files with the extensions `py`, `txt`, `md`, `ipynb`, ignoring the `experiments` and `__pycache__` directories, and ignoring the `old.py` file. 

Note that ignored directories are not included in the directory tree, but that ignored files are. However, the content of the ignored files will not be written to the final prompt under the `<files>` tag. This might be changed later...

In this example, the generated prompt will be saved as a `txt` file in the directory that `d2p` is called in with the name `project_prompt.txt`, and will have the following structure:

**<dir_name>_prompt.txt**
```xml
<context>
<directory_tree>
project/
├── README.md
├── requirements.txt
└── src/
    ├── __init__.py
    ├── main.py
    ├── old.py
    ├── production.ipynb
    └── testing.rs
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

<file>
<path>src/production.ipynb</path>
<content>
---------- Cell 1 (markdown) ----------
### Biologically inspired artificial neuron 

$$
y = mx + b
$$

---------- Cell 2 (code) ----------
def neuron(x, m, b):
    return m * x + b


</content>
</file>

</files>
</context>
```

You can then feed this prompt into an LLM to provide it with context about your project 🦾

## Config File 📋

`dir2prompt` uses a config file, `config.json`, to list common directories and files that should be ignored and excluded from the prompt. You can customize the behavior by supplying your own config file using the `--config` option:

**example.json**
```json
{
    "IGNORE_DIRS": [
        "experiments",
        "run*",
        ...
    ],
    "IGNORE_FILES": [
        ".pt",
        ".rs",
        ...
    ]
}
```





## License 📄

`dir2prompt` is released under the MIT License 🤓

## Contributing 🤝

Contributions are welcome! If you find any issues or have suggestions for improvements, please open an issue or submit a pull request on the GitHub repository.


