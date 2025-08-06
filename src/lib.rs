use std::{
    collections::HashSet,
    env,
    fs,
    path::{Path, PathBuf},
};

use arboard::Clipboard;
use clap::Parser;
use colored::Colorize;
use once_cell::sync::Lazy;
use pyo3::prelude::*;
use pyo3::wrap_pyfunction;
use serde::Deserialize;
// use yash_fnmatch::{without_escape, Pattern}; 

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
static DEFAULT_IGNORE: Lazy<IgnoreConfig> =
    Lazy::new(|| serde_json::from_str(DEFAULT_CONFIG).expect("embedded config.json is valid"));

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

    /// Only include the directory tree in the prompt and print it to the terminal
    #[arg(long = "tree", help = "Only include the directory tree in the prompt and print it to the terminal")]
    tree_only: bool,

    /// Copy the generated prompt to the clipboard
    #[arg(long = "cp", help = "Copy the generated prompt to the clipboard")]
    cp: bool,
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

// --- CLEANING LOGIC ---

/// Pass 1: Recursively find the names of all valid subdirectories.
fn collect_all_sub_dir_names(
    current_dir: &Path,
    ignore_dirs: &[String],
    names_set: &mut HashSet<String>,
) -> anyhow::Result<()> {
    if !current_dir.is_dir() {
        return Ok(());
    }

    for entry in fs::read_dir(current_dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            let name = path.file_name().unwrap_or_default().to_string_lossy();
            if !name.starts_with('.') && !ignore_dirs.contains(&name.to_string()) {
                if let Some(sub_name) = path.file_name().and_then(|s| s.to_str()) {
                    names_set.insert(sub_name.to_string());
                }
                // Recurse into the valid subdirectory
                collect_all_sub_dir_names(&path, ignore_dirs, names_set)?;
            }
        }
    }
    Ok(())
}


/// Pass 2: Recursively find all candidate prompt files.
fn find_all_prompts(
    current_dir: &Path,
    prompt_files: &mut Vec<PathBuf>,
) -> anyhow::Result<()> {
    if !current_dir.is_dir() {
        return Ok(());
    }

    for entry in fs::read_dir(current_dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            // Descend into all directories, even ignored ones, to find prompts.
            find_all_prompts(&path, prompt_files)?;
        } else if let Some(filename) = path.file_name().and_then(|s| s.to_str()) {
            if filename.ends_with("_prompt.txt") {
                prompt_files.push(path);
            }
        }
    }
    Ok(())
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

    if cli.clean {
        let start_path = Path::new(&cli.dir);
        if !start_path.is_dir() {
            anyhow::bail!("Invalid directory provided for cleaning: '{}'", cli.dir);
        }

        // Pass 1: Collect all valid directory names in the project.
        let mut valid_dir_names = HashSet::new();
        let root_name = start_path
            .canonicalize()?
            .file_name()
            .and_then(|s| s.to_str())
            .map(String::from)
            .ok_or_else(|| anyhow::anyhow!("Could not determine name of start directory"))?;
        valid_dir_names.insert(root_name);
        collect_all_sub_dir_names(start_path, &dir_ignore, &mut valid_dir_names)?;

        // Pass 2: Find all potential prompt files in the entire tree.
        let mut prompt_files_to_check = Vec::new();
        find_all_prompts(start_path, &mut prompt_files_to_check)?;

        let mut cleaned_count = 0;
        // Pass 3: Validate and delete.
        for file_path in prompt_files_to_check {
            if let Some(stem) = file_path.file_stem().and_then(|s| s.to_str()) {
                if let Some(base_name) = stem.strip_suffix("_prompt") {
                    if valid_dir_names.contains(base_name) {
                        fs::remove_file(&file_path)?;
                        println!("Removed {}", file_path.display().to_string().cyan());
                        cleaned_count += 1;
                    }
                }
            }
        }

        if cleaned_count == 0 {
            println!(
                "No matching prompt files found to clean starting from '{}'.",
                start_path.display().to_string().cyan()
            );
        }
        Ok(())
    } else {
        let root_path = Path::new(&cli.dir)
            .canonicalize()
            .map_err(|e| anyhow::anyhow!("invalid directory '{}': {}", &cli.dir, e))?;

        let dir_name = root_path
            .file_name()
            .ok_or_else(|| anyhow::anyhow!("invalid directory"))?
            .to_string_lossy()
            .to_string();

        let outfile = cli
            .outfile
            .unwrap_or_else(|| format!("{}_prompt", dir_name));

        // Generate the plain text prompt first.
        let prompt = build_prompt_internal(
            &cli.dir,
            &cli.filter,
            &dir_ignore,
            &merge(&config.files, &cli.ignore_files),
            cli.tree_only,
        )?;

        // If tree_only, print the plain text tree to the console.
        if cli.tree_only {
            println!("{}", prompt);
        }

        // If cp, copy the plain text prompt to the clipboard.
        if cli.cp {
            let mut clipboard = Clipboard::new()?;
            clipboard.set_text(&prompt)?;
            println!("{}", "Prompt copied to clipboard.".green());
        }

        // Save the plain text prompt to the file.
        let outpath = Path::new(&cli.outpath).join(format!("{outfile}.txt"));
        fs::write(&outpath, &prompt)?;
        println!("Prompt saved to {}", outpath.display().to_string().cyan());
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
#[pyo3(signature = (
    dir=".",
    filter=Vec::<String>::new(),
    ignore_dirs=Vec::<String>::new(),
    ignore_files=Vec::<String>::new(),
    tree_only=false
))]
fn build_prompt(
    dir: &str,
    filter: Vec<String>,
    ignore_dirs: Vec<String>,
    ignore_files: Vec<String>,
    tree_only: bool,
) -> PyResult<String> {
    build_prompt_internal(dir, &filter, &ignore_dirs, &ignore_files, tree_only)
        .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))
}

/// Shared implementation for CLI + Python call
fn build_prompt_internal(
    dir: &str,
    filter: &[String],
    ignore_dirs: &[String],
    ignore_files: &[String],
    tree_only: bool,
) -> anyhow::Result<String> {
    // 1. Prepare ignore lists
    let dir_path = Path::new(dir);
    let base = if dir == "." {
        env::current_dir()?
            .file_name()
            .unwrap()
            .to_string_lossy()
            .into_owned()
    } else {
        dir_path
            .file_name()
            .ok_or_else(|| anyhow::anyhow!("invalid directory"))?
            .to_string_lossy()
            .into_owned()
    };
    
    // Create a set of ignored file extensions for quick lookup, handling the leading dot.
    let ignore_exts: HashSet<String> = ignore_files
        .iter()
        .map(|s| s.strip_prefix('.').unwrap_or(s).to_lowercase())
        .collect();

    // To prevent including previously generated prompts, find all valid dir names
    // and add their corresponding prompt files to the ignore list.
    let mut dir_names: HashSet<String> = HashSet::new();
    dir_names.insert(base.clone());
    let sub_dirs = collect_dirs(dir_path, ignore_dirs)?;
    dir_names.extend(sub_dirs);

    let prompt_ignores: Vec<String> =
        dir_names.iter().map(|d| format!("{}_prompt.txt", d)).collect();
    
    let all_ignore_files = merge(ignore_files, &prompt_ignores);

    // 2. walk directory, collect files, render tree
    let mut tree = format!("{}/\n", base);
    let mut files = Vec::<PathBuf>::new();
    walk(
        dir_path,
        Path::new(""),
        "",
        ignore_dirs,
        &all_ignore_files,
        &ignore_exts,
        &mut tree,
        &mut files,
    )?;

    if tree_only {
        return Ok(tree);
    }

    // 3. stitch final prompt
    let mut prompt = String::from("<context>\n<directory_tree>\n");
    prompt.push_str(&tree);
    prompt.push_str("</directory_tree>\n\n<files>\n\n");

    for rel in files {
        let full = dir_path.join(&rel);
        if filter.is_empty()
            || filter
                .iter()
                .any(|f| rel.to_string_lossy().ends_with(f))
        {
            let content =
                fs::read_to_string(&full).unwrap_or_else(|_| "BINARY OR UNREADABLE".into());
            prompt.push_str(&format!(
                "<file>\n<path>{}</path>\n<content>\n{}\n</content>\n</file>\n\n",
                rel.display(),
                if content.trim().is_empty() {
                    "EMPTY FILE"
                } else {
                    &content
                }
            ));
        }
    }
    prompt.push_str("</files>\n</context>");
    Ok(prompt)
}

/// Collect non-ignored directory names
fn collect_dirs(abs: &Path, dir_ignores: &[String]) -> anyhow::Result<HashSet<String>> {
    let mut dirs = HashSet::new();
    if !abs.is_dir() {
        return Ok(dirs);
    }
    let mut entries: Vec<_> = fs::read_dir(abs)?
        .filter_map(|e| e.ok())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    entries.sort();

    for entry in entries {
        if entry.starts_with('.') {
            continue;
        }
        let abs_path = abs.join(&entry);
        if abs_path.is_dir() {
            if !dir_ignores.contains(&entry) {
                dirs.insert(entry.clone());
                let sub = collect_dirs(&abs_path, dir_ignores)?;
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
    ignore_dirs: &[String],
    ignore_files: &[String],
    ignore_exts: &HashSet<String>,
    tree: &mut String,
    files: &mut Vec<PathBuf>,
) -> anyhow::Result<()> {
    let mut visible_entries: Vec<String> = Vec::new();
    for entry_res in fs::read_dir(abs)? {
        if let Ok(dir_entry) = entry_res {
            let entry_name_os = dir_entry.file_name();
            let entry_name = entry_name_os.to_string_lossy();

            // --- IGNORE LOGIC ---

            // 1. Check for dotfiles, with exceptions for .env.example files.
            if entry_name.starts_with('.') 
                && entry_name != ".env.example" 
                && entry_name != ".example.env" {
                continue;
            }

            let abs_path = abs.join(entry_name.as_ref());
            let is_dir = abs_path.is_dir();

            // 2. Check against ignore lists using exact matches.
            let ignore = if is_dir {
                // Exact match for directory names
                ignore_dirs.contains(&entry_name.to_string())
            } else {
                // Exact match for full filename OR file extension
                ignore_files.contains(&entry_name.to_string()) ||
                abs_path.extension()
                    .and_then(|s| s.to_str())
                    .map(|ext| ignore_exts.contains(&ext.to_lowercase()))
                    .unwrap_or(false)
            };


            if !ignore {
                visible_entries.push(entry_name.into_owned());
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
                ignore_dirs,
                ignore_files,
                ignore_exts,
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

/// ----------  Python module entry-point  ----------
#[pymodule]
fn dir2prompt(_py: Python, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(build_prompt, m)?)?;
    m.add_function(wrap_pyfunction!(cli, m)?)?;
    Ok(())
}