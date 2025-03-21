use clap::Parser;
use glob::Pattern;
use ignore::WalkBuilder;
use std::collections::HashMap;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

/// CLI arguments
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Directory to scan
    #[arg(long, short, default_value = ".")]
    dir: String,

    /// Glob pattern to match files (e.g. "**/*.rs")
    #[arg(
        long,
        short,
        default_value = "**/*",
        conflicts_with = "files",
        help = "Glob pattern to match files (e.g. \"**/*.rs\")"
    )]
    pattern: String,

    /// List of specific files (space-separated)
    #[arg(
        long,
        short,
        num_args = 1..,
        conflicts_with = "pattern",
        help = "List of specific files (space-separated)"
    )]
    files: Vec<String>,
}

/// Determine the language for the code fence based on the file extension
fn determine_language(file_path: &str) -> String {
    let extension_to_language: HashMap<String, String> = HashMap::from([
        ("rs".to_string(), "rust".to_string()),
        ("go".to_string(), "golang".to_string()),
        ("py".to_string(), "python".to_string()),
        ("cpp".to_string(), "cpp".to_string()),
        ("c".to_string(), "c".to_string()),
        ("ts".to_string(), "typescript".to_string()),
    ]);

    file_path
        .rsplit('.')
        .next()
        .and_then(|ext| extension_to_language.get(ext).cloned())
        .unwrap_or_default()
}

/// Return true if the file should be excluded as a lock file
fn is_lock_file(path: &Path) -> bool {
    if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
        return name.ends_with(".lock")
            || name == "Cargo.lock"
            || name == "package-lock.json"
            || name == "yarn.lock"
            || name == "Pipfile.lock"
            || name == "poetry.lock";
    }
    false
}

/// Return true if the file should be excluded (lock file or dotfile or parent dir is hidden)
fn is_excluded(path: &Path) -> bool {
    if is_lock_file(path) {
        return true;
    }

    for component in path.components() {
        if let std::path::Component::Normal(part) = component {
            if let Some(part_str) = part.to_str() {
                if part_str.starts_with('.') {
                    return true;
                }
            }
        }
    }

    false
}

/// Check if a file is excluded by .gitignore using ignore::WalkBuilder
fn is_ignored_by_gitignore(base_dir: &Path, file_path: &Path) -> bool {
    let parent = file_path.parent().unwrap_or(base_dir);
    for result in WalkBuilder::new(parent)
        .standard_filters(true)
        .follow_links(true)
        .build()
    {
        if let Ok(entry) = result {
            if entry.path() == file_path {
                return false;
            }
        }
    }
    true // Didn't show up in walk = ignored
}

fn main() -> io::Result<()> {
    let args = Args::parse();

    let mut matched_files = Vec::new();

    if !args.files.is_empty() {
        for file in &args.files {
            let full_path = Path::new(&args.dir).join(file).canonicalize()?;

            if !full_path.exists() || !full_path.is_file() {
                eprintln!("Warning: '{}' is not a valid file.", full_path.display());
                continue;
            }

            if is_excluded(&full_path) {
                continue;
            }

            if is_ignored_by_gitignore(Path::new(&args.dir), &full_path) {
                continue;
            }

            matched_files.push(full_path);
        }
    } else {
        let pattern = Pattern::new(&args.pattern).unwrap_or_else(|e| {
            eprintln!("Invalid glob pattern: {e}");
            std::process::exit(1);
        });

        for result in WalkBuilder::new(&args.dir)
            .follow_links(true)
            .standard_filters(true)
            .build()
        {
            let entry = match result {
                Ok(e) => e,
                Err(e) => {
                    eprintln!("Error reading directory entry: {}", e);
                    continue;
                }
            };

            let path = entry.path();

            if entry.file_type().map_or(false, |ft| ft.is_file()) && !is_excluded(path) {
                let relative_path = path.strip_prefix(&args.dir).unwrap_or(path);
                let relative_path_str = relative_path.to_string_lossy();
                if pattern.matches(&relative_path_str) {
                    matched_files.push(path.to_path_buf());
                }
            }
        }
    }

    matched_files.sort();

    let mut output = Vec::new();

    for file_path in &matched_files {
        let content = match fs::read_to_string(file_path) {
            Ok(c) => c,
            Err(e) => {
                eprintln!("Error reading file '{}': {}", file_path.display(), e);
                continue;
            }
        };
        let language = determine_language(&file_path.to_string_lossy());
        writeln!(output, "```{}", language)?;
        writeln!(output, "// {}", file_path.display())?;
        write!(output, "{}", content)?;
        writeln!(output, "```")?;
        writeln!(output)?;
    }

    io::stdout().write_all(&output)?;

    Ok(())
}
