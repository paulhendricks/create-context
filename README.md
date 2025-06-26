# Create Context

This tool recursively scans a given directory, finds files matching a given glob pattern, concatenates their contents into a specified output format, and prints them to standard output. It’s designed to help you feed code into large language models with a uniform code-block format.

## Features

- Recursively walks a directory.
- Uses a glob pattern (like `**/*.rs`) to filter files.
- Concatenates matched files into a series of ```` ```rust ``` ```` code blocks.
- Prints all results to `stdout`.

## Installation

You need [Rust and Cargo](https://www.rust-lang.org/tools/install).

To install this binary from source:

```bash
git clone git@github.com:paulhendricks/create-context.git
cd create-context
cargo install --path .
cargo run -- --dir . --pattern '**/*.rs'
create-context --dir . --pattern '**/*.rs'
```

## Output

```bash
$ create-context --pattern "**/*" --dir ./examples --ignore-tests
```


Directory Structure:

```text
.
    └── example.rs

1 directories, 1 files
```

```rust
// ./examples/example.rs
fn main() {
    println!("Hello world!");
}
```


```bash
rg zmq -l .
rg zmq -l src | xargs create-context --dir . --files
```
