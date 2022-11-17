use humantime::format_rfc3339_seconds;
use ignore::WalkBuilder;
use rayon::prelude::{IntoParallelRefIterator, ParallelIterator};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;
mod stats;

#[derive(clap::Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Directory or file to parse
    #[arg(short, long, default_value = "./download/source")]
    source: PathBuf,
    /// CSV file to write output
    #[arg(short, long, default_value = "./output/syntax.csv")]
    output: PathBuf,

    #[arg(long, default_value_t = default_postfix())]
    postfix: String,
}

fn default_postfix() -> String {
    let now = SystemTime::now();
    let now_timestamp = format_rfc3339_seconds(now.into());

    let hostname = hostname::get().unwrap();
    let hostname = hostname.to_str().unwrap().split(".").next().unwrap_or("");

    format!("{now_timestamp}_{hostname}")
}

fn run_sources(source_path: &Path, args: &Args) {
    let source_paths: Vec<_> = fs::read_dir(source_path)
        .unwrap()
        .map(|d| d.unwrap())
        .filter(|d| d.file_type().unwrap().is_dir())
        .map(|d| d.path())
        .collect();

    source_paths.par_iter().for_each(|d| run_versions(d, args))
}

fn find_rust_files(path: &Path) -> impl Iterator<Item = PathBuf> {
    let mut builder = ignore::types::TypesBuilder::new();
    builder.add_defaults();
    builder.select("rust");
    let matcher = builder.build().unwrap();
    WalkBuilder::new(path)
        .types(matcher)
        .build()
        .map(|f| f.unwrap())
        .filter(|d| d.file_type().unwrap().is_file())
        .map(|d| d.into_path())
}

fn run_versions(source_path: &Path, args: &Args) {
    let crate_name = source_path
        .components()
        .last()
        .unwrap()
        .as_os_str()
        .to_string_lossy();

    let mut output = args.output.to_path_buf();
    let output_name = output
        .file_name()
        .expect("output must have a file name")
        .to_string_lossy();
    let mut output_name: Vec<_> = output_name.split('.').collect();
    if output_name.len() == 1 {
        output_name.push("csv");
    }
    output_name.insert(output_name.len() - 1, &crate_name);
    if !args.postfix.is_empty() {
        output_name.insert(output_name.len() - 1, &args.postfix);
    }
    output.set_file_name(output_name.join("."));

    let output = csv::Writer::from_path(output).unwrap();
    let mut stats = stats::traits::Stats::new(output);

    for path in find_rust_files(source_path) {
        let path = path.canonicalize().unwrap();
        let rel_path = path.strip_prefix(&source_path).unwrap();
        if rel_path.components().any(|s| {
            let s = s.as_os_str().to_str().unwrap();
            let s = s.trim_end_matches(".rs");
            s == "test" || s == "tests" || s == "test_data"
        }) {
            // Don't include test files
            continue;
        }

        stats.set_location(&crate_name, rel_path.display());

        if let Err(err) = stats.collect(&path) {
            eprintln!(
                "Error parsing {}:{}: {}",
                path.display(),
                err.span().start().line,
                err
            );
        } else {
            // eprintln!("Parsing {}", path.display());
        }
    }
}

fn main() {
    let args: Args = clap::Parser::parse();
    rayon::ThreadPoolBuilder::new()
        .stack_size(16 * 1024 * 1024)
        .build_global()
        .unwrap();
    run_sources(&args.source.canonicalize().unwrap(), &args);
}
