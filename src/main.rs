use chrono::Datelike;
use git2::Repository;
use humantime::format_rfc3339_seconds;
use ignore::WalkBuilder;
use rayon::prelude::{IntoParallelRefIterator, ParallelIterator};
use stats::traits::{Rows, Stats};
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

    /// Start date in mm-yyyy format.
    #[arg(long, value_parser = MonthYearParser, default_value = "05-2015")]
    start_date: (u32, u32),

    /// End date in mm-yyyy format.
    #[arg(long, value_parser = MonthYearParser, default_value = "11-2022")]
    end_date: (u32, u32),
}

#[derive(Debug, Clone)]
struct MonthYearParser;

impl clap::builder::TypedValueParser for MonthYearParser {
    type Value = (u32, u32);

    fn parse_ref(
        &self,
        cmd: &clap::Command,
        arg: Option<&clap::Arg>,
        value: &std::ffi::OsStr,
    ) -> Result<Self::Value, clap::Error> {
        let str = clap::builder::StringValueParser::new().parse_ref(cmd, arg, value)?;
        let err = Err(clap::Error::raw(
            clap::error::ErrorKind::InvalidValue,
            "must be formatted as mm-yyyy",
        )
        .with_cmd(cmd));
        let parts: Vec<_> = str.split('-').collect();
        let [m, y] = parts.as_slice() else { return err };
        let Ok(m) = m.parse::<u32>() else { return err };
        let Ok(y) = y.parse::<u32>() else { return err };
        Ok((m - 1, y))
    }
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
    let mut stats = Stats::new(output);

    let Ok(repo) = Repository::open(source_path) else {
        eprintln!("{} is not a git repository!", source_path.display());
        return;
    };

    let main_rev = repo.revparse_single("heads/main").unwrap();
    let mut repo_revwalk = repo.revwalk().unwrap();
    repo_revwalk.push(main_rev.id()).unwrap();
    repo_revwalk.set_sorting(git2::Sort::TIME).unwrap();

    let starting_idx = args.start_date.0 + args.start_date.1 * 12;
    let ending_idx = args.end_date.0 + args.end_date.1 * 12;
    let mut target_idx = ending_idx;

    for oid in repo_revwalk {
        if target_idx < starting_idx {
            break;
        }

        let oid = oid.unwrap();
        let commit = repo.find_commit(oid).unwrap();
        let seconds_since_epoch = commit.time().seconds();
        let time = chrono::NaiveDateTime::from_timestamp_opt(seconds_since_epoch, 0).unwrap();
        let current_idx = time.month0() + time.year_ce().1 * 12;

        while current_idx <= target_idx {
            let current_date = format!("{}-{}", current_idx % 12 + 1, current_idx / 12);
            println!("Checking out {crate_name} at {current_date}.");
            repo.checkout_tree(
                commit.as_object(),
                Some(git2::build::CheckoutBuilder::new().force()),
            )
            .unwrap();
            run_version(source_path, &mut stats, &crate_name, &current_date);
            target_idx -= 1;
        }
    }
}

fn run_version(source_path: &Path, stats: &mut Stats<impl Rows>, crate_name: &str, date_str: &str) {
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

        stats.set_location(&crate_name, rel_path.display(), date_str);

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
