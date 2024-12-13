use clap::Parser;
use glob::Pattern;
use std::collections::HashMap;
use std::fs;
use std::io::{self, Write};
use walkdir::WalkDir;

/// CLI arguments
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Directory to scan
    #[arg(long, short, default_value = ".")]
    dir: String,

    /// Glob pattern to match files (e.g. "**/*.rs")
    #[arg(long, short, default_value = "**/*")]
    pattern: String,
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
        .unwrap_or_default() // Default to no language if the extension is unrecognized
}

fn main() -> io::Result<()> {
    let args = Args::parse();

    // Compile the glob pattern
    let pattern = Pattern::new(&args.pattern).unwrap_or_else(|e| {
        eprintln!("Invalid glob pattern: {e}");
        std::process::exit(1);
    });

    // Recursively walk the directory and collect matching files
    let mut matched_files = Vec::new();
    for entry in WalkDir::new(&args.dir).follow_links(true) {
        let entry = match entry {
            Ok(e) => e,
            Err(e) => {
                eprintln!("Error reading directory entry: {}", e);
                continue;
            }
        };

        if entry.file_type().is_file() {
            let path_str = entry.path().to_string_lossy();
            if pattern.matches(&path_str) {
                matched_files.push(entry.path().to_path_buf());
            }
        }
    }

    // Sort files by path
    matched_files.sort();

    let mut output = Vec::new();

    for file_path in &matched_files {
        let content = fs::read_to_string(file_path)?;
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
