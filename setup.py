from setuptools import setup, find_packages

setup(
    name="dir2prompt",
    version="1.0.3",
    packages=find_packages(),
    package_data={
        "src": ["config.json"],   
    },
    entry_points={
        "console_scripts": [
            "d2p=src.d2p:main"
        ],
    },
    author="Max Kieffer",
    author_email="wkieffer@ufl.edu",
    description="Generate prompts for long-context LLMs using the content in your directory",
    long_description=open("README.md").read(),
    long_description_content_type="text/markdown",
    url="https://github.com/mkieffer1107/dir2prompt",
    license="MIT",
    keywords=[
        "prompt engineering",
        "large language model",
        "directory structure",
        "prompt generation",
        "file tree visualization",
        "directory to prompt",
        "automation tools",
        "developer utilities",
        "code documentation",
    ],
    python_requires=">=3.6",
    install_requires=[],
    classifiers=[
        "Development Status :: 3 - Alpha",
        "Intended Audience :: Developers",
        "License :: OSI Approved :: MIT License",
        "Programming Language :: Python :: 3",
        "Programming Language :: Python :: 3.6",
        "Programming Language :: Python :: 3.7",
        "Programming Language :: Python :: 3.8",
        "Programming Language :: Python :: 3.9",
    ],
)