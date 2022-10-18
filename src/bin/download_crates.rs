use anyhow::{anyhow, Error};
use clap::Parser;
use flate2::read::GzDecoder;
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
    #[arg(long)]
    update_crates_db: bool,
    #[arg(short = 'n', long, default_value_t = 100)]
    total_count: usize,
    #[arg(short = 'a', long, default_value = "1y")]
    max_age: humantime::Duration,
}

pub fn unpack_tar_gz(url: &str, into: &Path) -> Result<(), Error> {
    println!("Downloading {url:?}.");
    fs::create_dir_all(into)?;
    let read = ureq::get(url)
        .set("User-Agent", "syntax-sugar-survey (slsartor@wm.edu)")
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

pub fn main() -> Result<(), Error> {
    let Args {
        output,
        update_crates_db,
        total_count,
        max_age,
    } = Args::parse();
    let max_age: Duration = max_age.into();

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
    crates.retain(|row| row.updated_at > now - max_age);
    crates.sort_unstable_by_key(|row| Reverse(row.downloads));
    crates.truncate(total_count);

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

    // Download latest versions
    for row in &crates {
        let version = versions
            .get(&row.id)
            .and_then(|versions| versions.iter().max_by_key(|ver| &ver.created_at));

        let version = match version {
            Some(v) => v,
            None => continue,
        };

        if source_dir
            .join(format!("{0}-{1}", &row.name, &version.num))
            .exists()
        {
            // Already downloaded
            continue;
        }

        let url = format!(
            "https://static.crates.io/crates/{0}/{0}-{1}.crate",
            &row.name, &version.num
        );
        unpack_tar_gz(&url, &source_dir)?;
    }

    Ok(())
}
