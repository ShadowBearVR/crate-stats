use anyhow::{anyhow, bail, Error};
use clap::Parser;
use flate2::read::GzDecoder;
use indicatif::ProgressBar;
use std::cmp::Reverse;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};
use tar::Archive;

/// Download a bunch of rust crates
#[derive(Parser, Debug)]
#[command(name = "download_crates", author, version, about, long_about = None)]
struct Args {
    /// Directory to save output files
    #[arg(default_value = "./download")]
    output: PathBuf,
    #[arg(short = 'u', long)]
    update_crates_db: bool,
    #[arg(short = 'c', long, default_value_t = 100)]
    cratesio_count: usize,
    #[arg(short = 'g', long, default_value_t = 100)]
    github_count: usize,
    #[arg(long, default_value_t = 10)]
    github_db_pages: usize,
    #[arg(long, default_value_t = 100)]
    github_db_per_page: usize,
    #[arg(long)]
    github_token: Option<String>,
    #[arg(short = 'u', long)]
    update_github_db: bool,
    #[arg(short = 'a', long, default_value = "1y")]
    max_age: humantime::Duration,
}

const USER_AGENT: &str = "syntax-sugar-survey (slsartor@wm.edu)";

pub fn unpack_tar_gz(url: &str, into: &Path) -> Result<(), Error> {
    //println!("Downloading {url:?}.");
    fs::create_dir_all(into)?;
    let read = ureq::get(url)
        .set("User-Agent", USER_AGENT)
        .call()?
        .into_reader();
    let mut archive = Archive::new(GzDecoder::new(read));
    archive.unpack(into)?;
    Ok(())
}

#[derive(Debug, serde::Deserialize)]
pub struct CrateRow {
    pub id: u64,
    pub name: String,
    pub downloads: u32,
    #[serde(with = "humantime_serde")]
    pub updated_at: SystemTime,
    #[serde(with = "humantime_serde")]
    pub created_at: SystemTime,
}

#[derive(Debug, serde::Deserialize)]
pub struct VersionRow {
    pub crate_id: u64,
    pub num: String,
    pub license: String,
    pub yanked: char, // t or f
    #[serde(with = "humantime_serde")]
    pub created_at: SystemTime,
}

fn download_cratesio(
    &Args {
        cratesio_count,
        update_crates_db,
        max_age,
        ..
    }: &Args,
    output: &Path,
) -> Result<(), Error> {
    // Download a dump of crates.io if needed
    let db_dump = output.join("crates-db-dump");
    if !db_dump.exists() || update_crates_db {
        unpack_tar_gz("https://static.crates.io/db-dump.tar.gz", &db_dump)?;
    }

    // Find the last downloaded dump
    let db_dumps = db_dump
        .read_dir()?
        .map(|dir| match dir {
            Ok(dir) => Ok(dir.path()),
            Err(err) => Err(err),
        })
        .collect::<Result<Vec<PathBuf>, _>>()?;
    println!("Found crates.io database dumps: {db_dumps:#?}");
    let latest_dump = db_dumps
        .into_iter()
        .max_by(|a, b| a.file_name().cmp(&b.file_name()))
        .ok_or(anyhow!("no crates.io database dump downloaded"))?;

    // Read crates.csv
    let mut crates = csv::Reader::from_path(latest_dump.join("data").join("crates.csv"))?
        .into_deserialize()
        .collect::<Result<Vec<CrateRow>, _>>()?;

    // Filter down the list of crates
    let now = SystemTime::now();
    let max_age: Duration = max_age.into();
    crates.retain(|row| row.updated_at > now - max_age);
    crates.sort_unstable_by_key(|row| Reverse(row.downloads));
    crates.truncate(cratesio_count);

    // Create initial version lists
    let mut versions = HashMap::<u64, Vec<VersionRow>>::new();
    for row in &crates {
        versions.entry(row.id).or_default();
    }

    // Fill up the version lists
    for version in csv::Reader::from_path(latest_dump.join("data").join("versions.csv"))?
        .into_deserialize::<VersionRow>()
    {
        let version = version?;
        if version.yanked != 't' {
            if let Some(versions) = versions.get_mut(&version.crate_id) {
                versions.push(version);
            }
        }
    }

    let source_dir = output.join("source");
    let bar = ProgressBar::new(crates.len() as u64);

    // Download latest versions
    for row in &crates {
        let version = versions
            .get(&row.id)
            .and_then(|versions| versions.iter().max_by_key(|ver| &ver.created_at));

        let version = match version {
            Some(v) => v,
            None => continue,
        };

        let url = format!(
            "https://static.crates.io/crates/{0}/{0}-{1}.crate",
            &row.name, &version.num
        );

        if source_dir
            .join(format!("{0}-{1}", &row.name, &version.num))
            .exists()
        {
            // Already downloaded
        } else {
            unpack_tar_gz(&url, &source_dir)?;
        }

        bar.inc(1);
    }

    bar.finish();

    Ok(())
}

#[derive(Debug, serde::Deserialize, serde::Serialize)]
struct GitHubRepo {
    pub id: u64,
    pub name: String,
    pub full_name: String,
    #[serde(with = "humantime_serde")]
    pub updated_at: SystemTime,
    #[serde(with = "humantime_serde")]
    pub created_at: SystemTime,
    #[serde(with = "humantime_serde")]
    pub pushed_at: SystemTime,
    pub size: u64,
    pub stargazers_count: u64,
    pub forks_count: u64,
    pub open_issues_count: u64,
    pub default_branch: String,
    pub head: Option<String>,
}

fn search_github(
    &Args {
        ref github_token,
        github_db_pages: pages,
        github_db_per_page: per_page,
        ..
    }: &Args,
    output: &Path,
) -> Result<(), Error> {
    let github_token = match github_token {
        Some(t) => t,
        None => bail!("must provide github API token"),
    };

    let mut writer = csv::Writer::from_path(output)?;

    #[derive(serde::Deserialize)]
    struct SearchResults {
        items: Vec<GitHubRepo>,
    }

    #[derive(serde::Deserialize)]
    struct RefObject {
        sha: String,
    }

    #[derive(serde::Deserialize)]
    struct RefResult {
        object: RefObject,
    }

    let bar = ProgressBar::new(pages as u64 * per_page as u64);
    for page in 1..=pages {
        let url = format!("https://api.github.com/search/repositories?q=language:Rust&sort=stars&order=desc&page={page}&per_page={per_page}");
        let results: SearchResults = ureq::get(&url)
            .set("User-Agent", USER_AGENT)
            .set("Authorization", &format!("Bearer {github_token}"))
            .call()?
            .into_json()?;
        for mut repo in results.items {
            if repo.head.is_none() {
                let ref_url = format!(
                    "https://api.github.com/repos/{}/git/refs/heads/{}",
                    repo.full_name, repo.default_branch
                );
                let ref_result: RefResult = ureq::get(&ref_url)
                    .set("User-Agent", USER_AGENT)
                    .set("Authorization", &format!("Bearer {github_token}"))
                    .call()?
                    .into_json()?;
                repo.head = Some(ref_result.object.sha);
            }
            writer.serialize(&repo)?;
            bar.inc(1);
        }
    }

    Ok(())
}

fn download_github(args: &Args, output: &Path) -> Result<(), Error> {
    let latest_dump = output.join("github-search-dump.csv");
    if !latest_dump.is_file() || args.update_github_db {
        search_github(args, &latest_dump)?;
    }

    println!("Found GitHub search dump: {latest_dump:#?}");
    let mut repos = csv::Reader::from_path(&latest_dump)?
        .into_deserialize()
        .collect::<Result<Vec<GitHubRepo>, _>>()?;
    repos.sort_by_key(|repo| Reverse(repo.stargazers_count));
    repos.truncate(args.github_count);

    let source_dir = output.join("source");
    let bar = ProgressBar::new(repos.len() as u64);

    for repo in repos {
        let name = &repo.name;
        let full_name = &repo.full_name;
        let sha = match &repo.head {
            Some(s) => s,
            None => {
                eprintln!("Repo {full_name} is missing HEAD");
                continue;
            }
        };
        let url = format!("https://github.com/{full_name}/archive/{sha}.tar.gz");
        let source_name = format!("{name}-{sha}");
        if source_dir.join(&source_name).exists() {
            // Already downloaded
        } else {
            unpack_tar_gz(&url, &source_dir)?;
        }
        bar.inc(1);
    }

    Ok(())
}

pub fn main() -> Result<(), Error> {
    let args = Args::parse();
    download_cratesio(&args, &args.output)?;
    download_github(&args, &args.output)?;
    Ok(())
}
