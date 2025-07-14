use std::{
    env,
    fs,
    path::{Path, PathBuf},
    collections::HashSet,
};

use clap::Parser;
use once_cell::sync::Lazy;
use pyo3::prelude::*;
use pyo3::wrap_pyfunction;
use serde::Deserialize;
use yash_fnmatch::{without_escape, Pattern};

/// ----------  Config that used to live in config.json  ----------
static DEFAULT_CONFIG: &str = include_str!("config.json");

#[derive(Deserialize, Clone)]
struct IgnoreConfig {
    #[serde(rename = "IGNORE_DIRS")]
    dirs: Vec<String>,
    #[serde(rename = "IGNORE_FILES")]
    files: Vec<String>,
}

/// Lazily parse the default ignore lists once.
static DEFAULT_IGNORE: Lazy<IgnoreConfig> = Lazy::new(|| {
    serde_json::from_str(DEFAULT_CONFIG).expect("embedded config.json is valid")
});

/// ----------  Command-line interface  ----------
#[derive(Parser, Debug)]
#[command(name = "d2p", about = "Generate a prompt for a directory")]
struct Cli {
    /// Directory to scan
    #[arg(default_value = ".", help = "The directory to generate the prompt for")]
    dir: String,

    /// File-extension filters
    #[arg(long, num_args = 1.., help = "Filter for and process only files with these extensions (e.g., --filters py rs txt md)")]
    filter: Vec<String>,

    /// Additional directories to ignore
    #[arg(long = "ignore-dir", num_args = 1.., help = "Additional directories to ignore (e.g. --ignore-dir experiments __pycache__)")]
    ignore_dirs: Vec<String>,

    /// Additional files to ignore
    #[arg(long = "ignore-file", num_args = 1.., help = "Additional files or extensions to ignore (e.g. --ignore-file old.py rs)")]
    ignore_files: Vec<String>,

    /// Output path for prompt file
    #[arg(long, default_value = ".", hide_default_value = true,help = "The output path for the prompt file (default: current directory)")]
    outpath: String,

    /// Output file name for prompt file
    #[arg(long, help = "The name of the output file (default: <dir_name>_prompt)")]
    outfile: Option<String>,

    /// Path to custom config file
    #[arg(long, help = "Path to a custom configuration file (default: embedded config.json)")]
    config: Option<PathBuf>,

    /// Clean up all <folder>_prompt.txt files
    #[arg(long, help = "Remove all <folder>_prompt.txt files based on discovered directories")]
    clean: bool,
}

/// Exported for use in Python’s console-script stub.
#[pyfunction]
fn cli(py: Python<'_>) -> PyResult<()> {
    // Borrow sys.argv from Python so `d2p` behaves the same via pip or cargo run
    let sys = py.import_bound("sys")?;
    let argv: Vec<String> = sys.getattr("argv")?.extract()?;
    run_cli(argv.into_iter().skip(1))
        .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))
}

/// Real CLI body so we can call it from native tests too.
fn run_cli<I: IntoIterator<Item = String>>(raw_args: I) -> anyhow::Result<()> {
    // Insert dummy program name so clap parses flags correctly
    let mut args: Vec<String> = vec!["d2p".to_string()];
    args.extend(raw_args);

    let cli = Cli::parse_from(args);
    let config = cli
        .config
        .as_deref()
        .map(load_config)
        .transpose()?
        .unwrap_or_else(|| DEFAULT_IGNORE.clone());

    let dir_ignore = merge(&config.dirs, &cli.ignore_dirs);
    let dir_patterns = compile_patterns(&dir_ignore)?;

    if cli.clean {
        let dir_path = Path::new(&cli.dir);
        let base = if cli.dir == "." {
            env::current_dir()?.file_name().unwrap().to_string_lossy().into_owned()
        } else {
            dir_path
                .file_name()
                .ok_or_else(|| anyhow::anyhow!("invalid directory"))?
                .to_string_lossy()
                .into_owned()
        };
        let mut dir_names: HashSet<String> = HashSet::new();
        dir_names.insert(base);
        let sub_dirs = collect_dirs(dir_path, &dir_patterns)?;
        dir_names.extend(sub_dirs);

        for name in dir_names {
            let prompt_file = Path::new(&cli.outpath).join(format!("{}_prompt.txt", name));
            if prompt_file.exists() {
                fs::remove_file(&prompt_file)?;
                println!("Removed {}", prompt_file.display());
            }
        }
        Ok(())
    } else {
        let dir_path = Path::new(&cli.dir);
        let dir_name = if cli.dir == "." {
            env::current_dir()?.file_name().unwrap().to_string_lossy().to_string()
        } else {
            dir_path.file_name().ok_or_else(|| anyhow::anyhow!("invalid directory"))?.to_string_lossy().to_string()
        };
        let outfile = cli.outfile.unwrap_or_else(|| format!("{}_prompt", dir_name));

        let prompt = build_prompt_internal(
            &cli.dir,
            &cli.filter,
            &dir_ignore,
            &merge(&config.files, &cli.ignore_files),
        )?;

        let outpath = Path::new(&cli.outpath).join(format!("{outfile}.txt"));
        fs::write(&outpath, prompt)?;
        println!("Prompt saved to {}", outpath.display());
        Ok(())
    }
}

/// Helper: merge default + CLI ignore lists
fn merge(base: &[String], extra: &[String]) -> Vec<String> {
    let mut out = base.to_owned();
    out.extend(extra.iter().cloned());
    out
}

/// Optional external config file
fn load_config(path: &Path) -> anyhow::Result<IgnoreConfig> {
    Ok(serde_json::from_str(&fs::read_to_string(path)?)?)
}

/// ----------  Python-facing build_prompt()  ----------
#[pyfunction]
#[pyo3(signature = (dir=".", filter=Vec::<String>::new(), ignore_dirs=Vec::<String>::new(), ignore_files=Vec::<String>::new()))]
fn build_prompt(
    dir: &str,
    filter: Vec<String>,
    ignore_dirs: Vec<String>,
    ignore_files: Vec<String>,
) -> PyResult<String> {
    build_prompt_internal(dir, &filter, &ignore_dirs, &ignore_files)
        .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))
}

/// Shared implementation for CLI + Python call
fn build_prompt_internal(
    dir: &str,
    filter: &[String],
    ignore_dirs: &[String],
    ignore_files: &[String],
) -> anyhow::Result<String> {
    // 1. prepare ignore globs
    let dir_patterns = compile_patterns(ignore_dirs)?;
    let file_patterns = compile_patterns(ignore_files)?;

    // Collect all non-ignored directory names
    let dir_path = Path::new(dir);
    let base = if dir == "." {
        env::current_dir()?.file_name().unwrap().to_string_lossy().into_owned()
    } else {
        dir_path
            .file_name()
            .ok_or_else(|| anyhow::anyhow!("invalid directory"))?
            .to_string_lossy()
            .into_owned()
    };
    let mut dir_names: HashSet<String> = HashSet::new();
    dir_names.insert(base.clone());
    let sub_dirs = collect_dirs(dir_path, &dir_patterns)?;
    dir_names.extend(sub_dirs);

    // Create exact patterns for <dir>_prompt.txt
    let prompt_ignores: Vec<String> = dir_names.iter().map(|d| format!("{}_prompt.txt", d)).collect();
    let prompt_patterns = compile_patterns(&prompt_ignores)?;

    // 2. walk directory, collect files, render tree
    let mut tree = format!("{}/\n", base);
    let mut files = Vec::<PathBuf>::new();
    walk(
        dir_path,
        Path::new(""),
        "",
        &dir_patterns,
        &file_patterns,
        &prompt_patterns,
        &mut tree,
        &mut files,
    )?;

    // 3. stitch final prompt
    let mut prompt = String::from("<context>\n<directory_tree>\n");
    prompt.push_str(&tree);
    prompt.push_str("</directory_tree>\n\n<files>\n\n");

    for rel in files {
        let full = dir_path.join(&rel);
        if filter.is_empty() || filter.iter().any(|f| rel.to_string_lossy().ends_with(f)) {
            let content = fs::read_to_string(&full).unwrap_or_else(|_| "BINARY OR UNREADABLE".into());
            prompt.push_str(&format!(
                "<file>\n<path>{}</path>\n<content>\n{}\n</content>\n</file>\n\n",
                rel.display(),
                if content.trim().is_empty() { "EMPTY FILE" } else { &content }
            ));
        }
    }
    prompt.push_str("</files>\n</context>");
    Ok(prompt)
}

/// Collect non-ignored directory names
fn collect_dirs(abs: &Path, dir_pats: &[Pattern]) -> anyhow::Result<HashSet<String>> {
    let mut dirs = HashSet::new();
    let mut entries: Vec<_> = fs::read_dir(abs)?
        .filter_map(|e| e.ok())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    entries.sort();

    for entry in entries {
        if entry.starts_with(".") {
            continue;
        }
        let abs_path = abs.join(&entry);
        if abs_path.is_dir() {
            if !dir_pats.iter().any(|p| p.is_match(&entry)) {
                dirs.insert(entry.clone());
                let sub = collect_dirs(&abs_path, dir_pats)?;
                dirs.extend(sub);
            }
        }
    }
    Ok(dirs)
}

/// Walk directory recursively with proper indentation
fn walk(
    abs: &Path,
    rel: &Path,
    current_indent: &str,
    dir_pats: &[Pattern],
    file_pats: &[Pattern],
    prompt_pats: &[Pattern],
    tree: &mut String,
    files: &mut Vec<PathBuf>,
) -> anyhow::Result<()> {
    let mut visible_entries: Vec<String> = Vec::new();
    for entry_res in fs::read_dir(abs)? {
        if let Ok(dir_entry) = entry_res {
            let entry = dir_entry.file_name().to_string_lossy().into_owned();
            if entry.starts_with(".") {
                continue;
            }
            let abs_path = abs.join(&entry);
            let ignore = if abs_path.is_dir() {
                dir_pats.iter().any(|p| p.is_match(&entry))
            } else {
                file_pats.iter().any(|p| p.is_match(&entry)) || prompt_pats.iter().any(|p| p.is_match(&entry))
            };
            if !ignore {
                visible_entries.push(entry);
            }
        }
    }
    visible_entries.sort();

    for (i, entry) in visible_entries.iter().enumerate() {
        let is_last = i + 1 == visible_entries.len();
        let connector = if is_last { "└── " } else { "├── " };
        tree.push_str(current_indent);
        tree.push_str(connector);
        tree.push_str(entry);

        let abs_path = abs.join(entry);
        if abs_path.is_dir() {
            tree.push_str("/\n");
            let child_indent = format!("{}{}", current_indent, if is_last { "    " } else { "│   " });
            walk(
                &abs_path,
                &rel.join(entry),
                &child_indent,
                dir_pats,
                file_pats,
                prompt_pats,
                tree,
                files,
            )?;
        } else {
            tree.push('\n');
            files.push(rel.join(entry));
        }
    }
    Ok(())
}

/// Compile ignore patterns
fn compile_patterns(globs: &[String]) -> anyhow::Result<Vec<Pattern>> {
    globs
        .iter()
        .map(|g| Pattern::parse(without_escape(g)).map_err(|e| anyhow::anyhow!(e.to_string())))
        .collect()
}

/// ----------  Python module entry-point  ----------
#[pymodule]
fn dir2prompt(_py: Python, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(build_prompt, m)?)?;
    m.add_function(wrap_pyfunction!(cli, m)?)?;
    Ok(())
}