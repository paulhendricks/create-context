use clap::Parser;
use glob::Pattern;
use ignore::WalkBuilder;
use rayon::prelude::*;
use std::collections::HashMap;
use std::fs::{self, DirEntry};
use std::io::{self, Write};
use std::path::Path;
use tiktoken_rs::cl100k_base;

/// CLI arguments
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    #[arg(long, short, default_value = ".")]
    dir: String,

    #[arg(
        long,
        short,
        default_value = "**/*",
        conflicts_with = "files",
        help = "Glob pattern to match files (e.g. \"**/*.rs\")"
    )]
    pattern: String,

    #[arg(
        long,
        short,
        num_args = 1..,
        conflicts_with = "pattern",
        help = "List of specific files (space-separated)"
    )]
    files: Vec<String>,

    #[arg(long, help = "Disable printing of directory tree structure")]
    no_tree: bool,

    #[arg(long, help = "Enable parallel processing of file contents")]
    parallel: bool,

    #[arg(long, help = "Count and print the number of tokens in output")]
    count_tokens: bool,
}

fn determine_language(file_path: &str) -> String {
    let filename_to_language: HashMap<&str, &str> = HashMap::from([
        ("Makefile", "make"),
        ("CMakeLists.txt", "cmake"),
        ("Dockerfile", "docker"),
        (".gitignore", "git"),
        ("build.gradle", "gradle"),
        ("Cargo.toml", "rust"),
        ("package.json", "node"),
    ]);

    let extension_to_language: HashMap<&str, &str> = HashMap::from([
        ("rs", "rust"),
        ("zig", "zig"),
        ("zon", "zig"),
        ("go", "go"),
        ("py", "python"),
        ("cpp", "cpp"),
        ("cc", "cpp"),
        ("cxx", "cpp"),
        ("hpp", "cpp"),
        ("hh", "cpp"),
        ("hxx", "cpp"),
        ("c", "c"),
        ("h", "c"),
        ("cu", "cuda"),
        ("cuh", "cuda"),
        ("js", "javascript"),
        ("ts", "typescript"),
        ("toml", "toml"),
        ("yaml", "yaml"),
        ("yml", "yaml"),
        ("json", "json"),
        ("txt", "txt"),
        ("sh", "bash"),
        ("md", "markdown"),
        ("proto", "protobuf"),
        ("cmake", "cmake"),
        ("html", "html"),
        ("css", "css"),
    ]);

    let path = Path::new(file_path);

    if let Some(file_name) = path.file_name().and_then(|f| f.to_str()) {
        if let Some(lang) = filename_to_language.get(file_name) {
            return lang.to_string();
        }
    }

    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
        if let Some(lang) = extension_to_language.get(ext) {
            return lang.to_string();
        }
    }

    String::new()
}

fn comment_syntax(language: &str) -> (&'static str, Option<&'static str>) {
    match language {
        "rust" | "cpp" | "c" | "go" | "javascript" | "typescript" | "java" | "swift" | "kotlin" => {
            ("//", None)
        }
        "python" | "bash" | "sh" | "yaml" | "yml" | "toml" | "make" => ("#", None),
        "lua" => ("--", None),
        "html" | "xml" => ("<!--", Some("-->")),
        "css" | "scss" => ("/*", Some("*/")),
        "json" | "protobuf" => ("//", None),
        "markdown" => ("<!--", Some("-->")),
        _ => ("//", None),
    }
}

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

fn is_excluded(path: &Path, base_dir: &Path) -> bool {
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

    is_ignored_by_gitignore(base_dir, path)
}

fn is_ignored_by_gitignore(base_dir: &Path, file_path: &Path) -> bool {
    let parent = file_path.parent().unwrap_or(base_dir);
    for entry in WalkBuilder::new(parent)
        .standard_filters(true)
        .follow_links(true)
        .build()
        .flatten()
    {
        if entry.path() == file_path {
            return false;
        }
    }
    true
}

fn tree_entry_sort(a: &DirEntry, b: &DirEntry) -> std::cmp::Ordering {
    let a_is_dir = a.path().is_dir();
    let b_is_dir = b.path().is_dir();
    match (a_is_dir, b_is_dir) {
        (true, false) => std::cmp::Ordering::Less,
        (false, true) => std::cmp::Ordering::Greater,
        _ => a.file_name().cmp(&b.file_name()),
    }
}

fn walk_tree(
    dir: &Path,
    prefix: String,
    is_last: bool,
    dir_count: &mut usize,
    file_count: &mut usize,
    output: &mut Vec<String>,
    root: &Path,
) -> io::Result<()> {
    let connector = if is_last { "└── " } else { "├── " };
    if prefix.is_empty() {
        output.push(".".to_string());
    } else if let Some(name) = dir.file_name() {
        output.push(format!("{prefix}{connector}{}", name.to_string_lossy()));
    }

    let mut entries = fs::read_dir(dir)?
        .filter_map(Result::ok)
        .filter(|e| !is_excluded(&e.path(), root))
        .collect::<Vec<_>>();

    entries.sort_by(tree_entry_sort);

    let len = entries.len();
    for (i, entry) in entries.into_iter().enumerate() {
        let path = entry.path();
        let is_last_entry = i == len - 1;
        let new_prefix = format!("{prefix}{}", if is_last { "    " } else { "│   " });

        if path.is_dir() {
            *dir_count += 1;
            walk_tree(
                &path,
                new_prefix,
                is_last_entry,
                dir_count,
                file_count,
                output,
                root,
            )?;
        } else {
            *file_count += 1;
            let conn = if is_last_entry {
                "└── "
            } else {
                "├── "
            };
            output.push(format!(
                "{new_prefix}{conn}{}",
                entry.file_name().to_string_lossy()
            ));
        }
    }

    Ok(())
}

fn print_tree_structure(root: &Path) -> io::Result<()> {
    let mut dir_count = 1;
    let mut file_count = 0;
    let mut lines = Vec::new();
    walk_tree(
        root,
        "".to_string(),
        true,
        &mut dir_count,
        &mut file_count,
        &mut lines,
        root,
    )?;

    println!("Directory Structure:\n");
    println!("```text");
    for line in lines {
        println!("{line}");
    }
    println!("\n{} directories, {} files", dir_count, file_count);
    println!("```");

    Ok(())
}

fn process_file(file_path: &Path) -> Option<(String, String)> {
    let content = fs::read_to_string(file_path).ok()?;
    let language = determine_language(&file_path.to_string_lossy());
    let (start, end) = comment_syntax(&language);
    let mut buf = String::new();
    use std::fmt::Write;

    writeln!(buf, "```{}", language).ok()?;
    if let Some(end) = end {
        writeln!(buf, "{} {} {}", start, file_path.display(), end).ok()?;
    } else {
        writeln!(buf, "{} {}", start, file_path.display()).ok()?;
    }
    write!(buf, "{}", content).ok()?;
    writeln!(buf, "```").ok()?;
    writeln!(buf).ok()?;

    Some((file_path.to_string_lossy().to_string(), buf))
}

/// Count tokens using the cl100k_base tokenizer (OpenAI GPT-4 / GPT-3.5)
fn count_tokens(text: &str) -> usize {
    let bpe = cl100k_base().expect("Failed to load tokenizer");
    bpe.encode_with_special_tokens(text).len()
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

            if is_excluded(&full_path, Path::new(&args.dir)) {
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

            if entry.file_type().is_some_and(|ft| ft.is_file())
                && !is_excluded(path, Path::new(&args.dir))
            {
                let relative_path = path.strip_prefix(&args.dir).unwrap_or(path);
                let relative_path_str = relative_path.to_string_lossy();
                if pattern.matches(&relative_path_str) {
                    matched_files.push(path.to_path_buf());
                }
            }
        }
    }

    matched_files.sort();

    if !args.no_tree {
        print_tree_structure(Path::new(&args.dir))?;
        println!();
    }

    let outputs: Vec<(String, String)> = if args.parallel {
        matched_files
            .par_iter()
            .filter_map(|file_path| process_file(file_path))
            .collect()
    } else {
        matched_files
            .iter()
            .filter_map(|file_path| process_file(file_path))
            .collect()
    };

    let mut outputs = outputs;
    outputs.sort_by(|a, b| a.0.cmp(&b.0));

    let mut final_output = Vec::new();
    for (_, chunk) in outputs {
        write!(final_output, "{}", chunk)?;
    }

    if args.count_tokens {
        let output_str = String::from_utf8_lossy(&final_output);
        let token_count = count_tokens(&output_str);
        eprintln!("Token count: {}", token_count);
    } else {
        io::stdout().write_all(&final_output)?;
    }

    Ok(())
}
